//! Ghost engine: platform-agnostic orchestration layer.
//! Manages recording, element lookup, and replay with cancellation support.

use crate::auth::AuthManager;
use crate::config::GhostConfig;
use crate::core::ai::WorkflowAnalysis;
use crate::core::ai::WorkflowAnalyzer;
use crate::core::events::{
    ElementInfo, InputEvent, KeyAction, VisualCheckPoint, WaitCondition, Workflow, WorkflowMetadata,
};
use crate::core::execution::ExecutionHistory;
use crate::core::knowledge::{KnowledgeBase, LearnedPattern, ProactiveSuggestion};
use crate::core::llm::{self, LLMConfig};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use crate::core::vision;
use crate::core::wait::smart_wait;
use crate::performance::{PerformanceMonitor, PerformanceSummary};
use crate::storage;
use crate::telemetry::{TelemetryManager, UsageStats};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::DynamicImage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

fn workflow_file_path(workflows_dir: &std::path::Path, name: &str) -> anyhow::Result<PathBuf> {
    let safe_name = crate::core::security::sanitize_workflow_path(name)?;
    Ok(workflows_dir.join(safe_name).with_extension("json"))
}

/// One-shot approval that authorizes `replay_workflow` for a specific event list.
struct PendingRoutineApproval {
    fingerprint: String,
    expires_at: Instant,
}

const ROUTINE_APPROVAL_TTL: Duration = Duration::from_secs(300);

/// Half-width of the screenshot crop captured around a recorded click for
/// `ElementInfo::template_png` — 64x64 total, small enough to keep workflow
/// files a reasonable size while covering a typical button/field.
const TEMPLATE_CROP_HALF: i32 = 32;

/// Crop a small region around `(x, y)` from a fresh screenshot and PNG-encode
/// it, for the template-match replay fallback (`core::template_match`).
/// Best-effort: any failure (capture, decode, degenerate crop bounds) yields
/// `None` rather than interrupting recording.
fn capture_template_crop(x: i32, y: i32) -> Option<Box<[u8]>> {
    use image::GenericImageView;

    let bytes = vision::capture_screenshot().ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let (w, h) = img.dimensions();
    let x0 = (x - TEMPLATE_CROP_HALF).max(0) as u32;
    let y0 = (y - TEMPLATE_CROP_HALF).max(0) as u32;
    let x1 = ((x + TEMPLATE_CROP_HALF).max(0) as u32).min(w);
    let y1 = ((y + TEMPLATE_CROP_HALF).max(0) as u32).min(h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let crop = img.crop_imm(x0, y0, x1 - x0, y1 - y0);
    let mut buf = std::io::Cursor::new(Vec::new());
    crop.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some(buf.into_inner().into_boxed_slice())
}

/// RAII guard marking a replay as active for its lifetime (drop-safe, so the
/// flag clears even if the replay errors or panics).
struct ReplayActiveGuard(Arc<AtomicBool>);

impl ReplayActiveGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Relaxed);
        ReplayActiveGuard(flag)
    }
}

impl Drop for ReplayActiveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Main engine struct that holds platform-specific backends.
pub struct GhostEngine {
    recorder: Box<dyn InputRecorder>,
    locator: Box<dyn ElementLocator>,
    replayer: Box<dyn ReplayEngine>,
    /// Channel sender for recording events
    tx: Mutex<Option<mpsc::Sender<InputEvent>>>,
    /// Receiver stored for the bridge thread to consume
    rx: Mutex<Option<mpsc::Receiver<InputEvent>>>,
    /// Atomic flag for instant replay cancellation
    replay_stop_flag: Arc<AtomicBool>,
    /// True only while a replay is actually executing
    replay_active: Arc<AtomicBool>,
    /// Playback speed factor (1.0 = normal)
    playback_speed: Arc<Mutex<f32>>,
    /// Pause state for replay
    replay_paused: Arc<AtomicBool>,
    /// Live per-step progress of the running replay (polled by the UI)
    replay_progress: Arc<crate::core::replay_support::ReplayProgress>,
    /// Index of the event the last replay failed on, if it failed
    last_failed_step: Arc<Mutex<Option<usize>>>,
    /// Recorded events buffer
    recorded_events: Arc<Mutex<Vec<InputEvent>>>,
    /// AI workflow analyzer
    analyzer: WorkflowAnalyzer,
    /// Execution history tracker
    execution_tracker: Arc<Mutex<Option<ExecutionHistory>>>,
    /// Knowledge base for Smart Observer Mode
    knowledge_base: KnowledgeBase,
    /// Persisted user configuration (source of truth for runtime defaults)
    config: Arc<Mutex<GhostConfig>>,
    /// Opt-in usage telemetry (gated by config.privacy.telemetry_enabled)
    telemetry: Arc<TelemetryManager>,
    /// Opt-in performance monitor (gated by config.performance.profiling_enabled)
    perf: Arc<PerformanceMonitor>,
    /// Wall-clock start of the active recording session, for duration telemetry
    recording_start: Arc<Mutex<Option<Instant>>>,
    /// Local login + at-rest encryption for workflow data
    auth: Arc<AuthManager>,
    /// Linked Microsoft/Google account (sign-in identity), separate from `auth`
    accounts: Arc<crate::accounts::AccountManager>,
    /// One-shot approval for routine replay (policy plan → user approve → consume).
    routine_approval: Mutex<Option<PendingRoutineApproval>>,
}

impl GhostEngine {
    /// Create a new GhostEngine with the appropriate platform backend.
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        let (recorder, locator, replayer) = {
            use crate::platform::macos::MacosBackend;
            (
                MacosBackend::recorder(),
                MacosBackend::locator(),
                MacosBackend::replayer(),
            )
        };

        #[cfg(target_os = "windows")]
        let (recorder, locator, replayer) = {
            use crate::platform::windows::WindowsBackend;
            (
                WindowsBackend::recorder(),
                WindowsBackend::locator(),
                WindowsBackend::replayer(),
            )
        };

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let (recorder, locator, replayer) = {
            use crate::platform::headless::HeadlessBackend;
            (
                HeadlessBackend::recorder(),
                HeadlessBackend::locator(),
                HeadlessBackend::replayer(),
            )
        };

        // Load persisted config (falling back to defaults) and use it to seed
        // runtime state: starting playback speed and the active LLM provider.
        let config = GhostConfig::load().unwrap_or_default();
        let initial_speed = config.replay.default_speed.max(0.1);
        llm::init_llm(&LLMConfig::from_ghost_config(&config.ai));

        // Observability is opt-in: both honor the persisted privacy/performance
        // flags and no-op until enabled, so default runs collect nothing.
        let telemetry = Arc::new(TelemetryManager::new(config.privacy.telemetry_enabled));
        let perf = Arc::new(PerformanceMonitor::new(
            config.performance.profiling_enabled,
        ));

        GhostEngine {
            recorder,
            locator,
            replayer,
            tx: Mutex::new(None),
            rx: Mutex::new(None),
            replay_stop_flag: Arc::new(AtomicBool::new(false)),
            replay_active: Arc::new(AtomicBool::new(false)),
            playback_speed: Arc::new(Mutex::new(initial_speed)),
            replay_paused: Arc::new(AtomicBool::new(false)),
            replay_progress: Arc::new(crate::core::replay_support::ReplayProgress::default()),
            last_failed_step: Arc::new(Mutex::new(None)),
            recorded_events: Arc::new(Mutex::new(Vec::new())),
            analyzer: WorkflowAnalyzer::new(),
            execution_tracker: Arc::new(Mutex::new(ExecutionHistory::new().ok())),
            knowledge_base: KnowledgeBase::new(),
            config: Arc::new(Mutex::new(config)),
            telemetry,
            perf,
            recording_start: Arc::new(Mutex::new(None)),
            auth: Arc::new(AuthManager::new()),
            accounts: Arc::new(crate::accounts::AccountManager::new()),
            routine_approval: Mutex::new(None),
        }
    }

    /// Access the local auth manager (login + workflow encryption).
    pub fn auth(&self) -> Arc<AuthManager> {
        Arc::clone(&self.auth)
    }

    /// Access the linked-account manager (Microsoft/Google sign-in identity).
    pub fn accounts(&self) -> Arc<crate::accounts::AccountManager> {
        Arc::clone(&self.accounts)
    }

    /// Record a one-shot approval for the given event fingerprint (TTL-bound).
    pub fn store_routine_approval(&self, fingerprint: String) {
        *self.routine_approval.lock().unwrap() = Some(PendingRoutineApproval {
            fingerprint,
            expires_at: Instant::now() + ROUTINE_APPROVAL_TTL,
        });
    }

    /// Consume a matching unexpired approval, or error if missing/stale/mismatched.
    pub fn consume_routine_approval(&self, fingerprint: &str) -> Result<(), String> {
        let mut slot = self.routine_approval.lock().unwrap();
        match slot.take() {
            Some(pending)
                if pending.fingerprint == fingerprint && pending.expires_at > Instant::now() =>
            {
                Ok(())
            }
            Some(_) => Err(
                "Routine replay approval does not match these events (or expired). Re-approve the policy plan, then replay."
                    .into(),
            ),
            None => Err(
                "Routine replay requires an approved policy plan. Review the plan and approve before replaying."
                    .into(),
            ),
        }
    }

    /// Test-only engine with its `AuthManager` rooted at `auth_path` instead
    /// of the real OS data directory, so auth command tests can call
    /// `auth_setup`/`auth_unlock` without ever touching a developer's real
    /// local password file. The linked-account store is rooted at a sibling
    /// path derived from `auth_path` (not a shared `account.json`) so parallel
    /// command tests cannot leak sign-in state into each other.
    #[cfg(test)]
    pub(crate) fn with_auth_path(auth_path: PathBuf) -> Self {
        let mut engine = Self::new();
        let accounts_path = auth_path.with_extension("identity.json");
        engine.auth = Arc::new(AuthManager::with_path(auth_path));
        engine.accounts = Arc::new(crate::accounts::AccountManager::with_path(accounts_path));
        engine
    }

    /// Start recording input events. Events will be sent through the provided channel.
    pub fn start_recording(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        // Refuse to start a second session while one is active. Overwriting the
        // channel here would orphan the running recorder thread and leak the
        // OS-level event tap / hook (the CGEventTap lifecycle is stateful).
        if self.tx.lock().unwrap().is_some() {
            anyhow::bail!("Recording already active; stop the current session first");
        }

        // Clear previous recorded events
        *self.recorded_events.lock().unwrap() = Vec::new();

        // Store the sender and receiver for later use
        let (tx_clone, rx) = mpsc::channel();
        *self.tx.lock().unwrap() = Some(tx_clone);
        *self.rx.lock().unwrap() = Some(rx);

        // Mark the session start so stop_recording can report its duration.
        *self.recording_start.lock().unwrap() = Some(Instant::now());

        // If the OS-level recorder fails to start, roll the session state back so
        // the user can retry (otherwise the guard above would wrongly report an
        // "already active" session forever).
        if let Err(e) = self.recorder.start(tx) {
            *self.tx.lock().unwrap() = None;
            *self.rx.lock().unwrap() = None;
            *self.recording_start.lock().unwrap() = None;
            return Err(e);
        }
        Ok(())
    }

    /// Stop the active recording session.
    pub fn stop_recording(&self) {
        self.recorder.stop();
        *self.tx.lock().unwrap() = None;
        *self.rx.lock().unwrap() = None;

        // Report the completed recording to telemetry (no-op unless opted in).
        if let Some(started) = self.recording_start.lock().unwrap().take() {
            let event_count = self.recorded_events.lock().unwrap().len();
            self.telemetry
                .track_workflow_recorded(event_count, started.elapsed().as_secs());
        }
    }

    /// Add an event to the recorded events buffer (called from the bridge
    /// thread). When `performance.capture_element_templates` is enabled, a
    /// click's `ElementInfo` gets a small screenshot crop attached for the
    /// template-match replay fallback (`core::template_match`) — best-effort:
    /// a capture failure never blocks or corrupts recording.
    pub fn buffer_event(&self, mut event: InputEvent) {
        if let InputEvent::MouseClick {
            x,
            y,
            element: Some(el),
            ..
        } = &mut event
        {
            if el.template_png.is_none()
                && self
                    .config
                    .lock()
                    .unwrap()
                    .performance
                    .capture_element_templates
            {
                el.template_png = capture_template_crop(*x, *y);
            }
        }
        self.recorded_events.lock().unwrap().push(event);
    }

    /// Get all recorded events
    pub fn get_recorded_events(&self) -> Vec<InputEvent> {
        self.recorded_events.lock().unwrap().clone()
    }

    /// Replay a sequence of recorded events.
    ///
    /// `workflow_name` labels the run in execution history; pass `None` for an
    /// unsaved workflow.
    pub fn replay(
        &self,
        events: &[InputEvent],
        workflow_name: Option<String>,
    ) -> anyhow::Result<()> {
        // Reset the stop flag, pause state, and step progress before starting
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);
        self.replay_progress.begin(events.len());
        *self.last_failed_step.lock().unwrap() = None;

        let label = workflow_name
            .clone()
            .unwrap_or_else(|| "Unsaved workflow".to_string());
        let fingerprint = crate::policy::fingerprint_events(events);
        let wal = storage::open_default().ok().and_then(|db| {
            let id =
                storage::replay_runs::begin_replay_run(&db, &label, &fingerprint, events.len())
                    .ok()?;
            Some((std::sync::Arc::new(db), id))
        });
        if let Some((db, id)) = &wal {
            let db_cb = db.clone();
            let id_cb = id.clone();
            self.replay_progress.begin_wal(events.len(), move |report| {
                let _ = storage::replay_runs::update_replay_progress(&db_cb, &id_cb, report);
            });
        }

        // Time and record the replay (both no-op unless the user opted in).
        let started = Instant::now();
        self.perf.start_timer("replay");
        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        let result = self.replayer.execute(
            events,
            self.replay_stop_flag.clone(),
            self.replay_paused.clone(),
            self.get_playback_speed(),
            self.replay_progress.clone(),
        );
        drop(_active);
        self.record_failed_step(&result);
        self.perf.stop_timer("replay");
        self.telemetry.track_workflow_replayed(
            events.len(),
            started.elapsed().as_secs(),
            result.is_ok(),
        );

        if let Some((db, id)) = wal {
            let cancelled = self.replay_stop_flag.load(Ordering::Relaxed);
            if !cancelled && result.is_ok() {
                if let Ok(stored) = storage::replay_runs::get_replay_run(&db, &id) {
                    let report = crate::audit::ReplayRunReport {
                        events_applied: stored.events_applied,
                        events_total: stored.events_total,
                        undo: stored.undo,
                    };
                    let _ = storage::replay_runs::finish_replay_run(&db, &id, &report);
                }
            }
            self.replay_progress.clear_wal();
        }

        self.record_replay(workflow_name, events.len(), started, &result);

        result
    }

    /// Persist an execution record for a finished replay so success-rate,
    /// duration, and failure analytics reflect real runs. Best-effort: a
    /// failure to write history must never fail the replay itself.
    fn record_replay(
        &self,
        workflow_name: Option<String>,
        events_processed: usize,
        started: Instant,
        result: &anyhow::Result<()>,
    ) {
        use crate::core::execution::{build_replay_record, ReplayOutcome};

        // The platform replayers return Ok(()) on user cancel, so consult the
        // stop flag to distinguish a cancelled run from a completed one.
        let outcome = if self.replay_stop_flag.load(Ordering::Relaxed) {
            ReplayOutcome::Cancelled
        } else {
            match result {
                Ok(()) => ReplayOutcome::Completed,
                Err(e) => ReplayOutcome::Failed(e.to_string()),
            }
        };

        let record = build_replay_record(
            workflow_name,
            events_processed,
            started.elapsed().as_millis() as u64,
            self.get_playback_speed(),
            outcome,
            self.replay_progress.take_trace(),
        );

        if let Ok(guard) = self.execution_tracker.lock() {
            if let Some(history) = guard.as_ref() {
                if let Err(e) = history.save(&record) {
                    tracing::warn!("Failed to save replay execution record: {e}");
                }
            }
        }
    }

    /// After a replay finishes, remember which event it failed on (if any) so
    /// the UI can mark the step and offer "retry from failed step".
    fn record_failed_step(&self, result: &anyhow::Result<()>) {
        if result.is_err() {
            let (current, _) = self.replay_progress.snapshot();
            *self.last_failed_step.lock().unwrap() = Some(current);
        }
    }

    /// Snapshot live replay progress: (current step, total steps, failed step).
    /// `failed step` refers to the most recent finished replay and is cleared
    /// when a new replay starts.
    pub fn get_replay_progress(&self) -> (usize, usize, Option<usize>) {
        let (current, total) = self.replay_progress.snapshot();
        let failed = *self.last_failed_step.lock().unwrap();
        (current, total, failed)
    }

    /// Cancel an ongoing replay immediately.
    pub fn cancel_replay(&self) {
        self.replay_stop_flag.store(true, Ordering::Relaxed);
    }

    /// Pause an ongoing replay.
    pub fn pause_replay(&self) {
        self.replay_paused.store(true, Ordering::Relaxed);
    }

    /// Resume a paused replay.
    pub fn resume_replay(&self) {
        self.replay_paused.store(false, Ordering::Relaxed);
    }

    /// Check if replay is currently paused.
    pub fn is_replay_paused(&self) -> bool {
        self.replay_paused.load(Ordering::Relaxed)
    }

    /// Set the playback speed factor.
    pub fn set_playback_speed(&self, factor: f32) {
        *self.playback_speed.lock().unwrap() = factor.max(0.1);
    }

    /// Get the current playback speed factor.
    pub fn get_playback_speed(&self) -> f32 {
        *self.playback_speed.lock().unwrap()
    }

    /// Snapshot the current persisted configuration.
    pub fn get_config(&self) -> GhostConfig {
        self.config.lock().unwrap().clone()
    }

    /// Validate, persist, and apply a new configuration. Re-seeds the live
    /// playback speed and rebuilds the active LLM provider so changes take
    /// effect without a restart.
    pub fn update_config(&self, new_config: GhostConfig) -> anyhow::Result<()> {
        new_config.validate()?;
        new_config.save()?;

        *self.playback_speed.lock().unwrap() = new_config.replay.default_speed.max(0.1);
        llm::init_llm(&LLMConfig::from_ghost_config(&new_config.ai));

        // Honor opt-in toggles live, mirroring the playback-speed/LLM re-seed above.
        self.telemetry
            .set_enabled(new_config.privacy.telemetry_enabled);
        self.perf
            .set_enabled(new_config.performance.profiling_enabled);

        *self.config.lock().unwrap() = new_config;
        Ok(())
    }

    /// Snapshot the collected usage telemetry statistics.
    pub fn get_telemetry_stats(&self) -> UsageStats {
        self.telemetry.get_stats()
    }

    /// Export all collected telemetry (session id, stats, events) as JSON.
    pub fn export_telemetry(&self) -> anyhow::Result<String> {
        Ok(self.telemetry.export_json()?)
    }

    /// Summarize recorded performance metrics by operation.
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        self.perf.get_summary()
    }

    /// Record a feature-usage event (no-op unless telemetry is enabled).
    pub fn track_feature(&self, feature: &str) {
        self.telemetry.track_feature_used(feature);
    }

    /// Build a default retry config from the persisted replay settings.
    pub fn default_retry_config(&self) -> crate::core::events::RetryConfig {
        let replay = &self.config.lock().unwrap().replay;
        crate::core::events::RetryConfig {
            max_attempts: replay.max_retry_attempts,
            backoff_ms: replay.retry_backoff_ms,
            backoff_multiplier: replay.retry_backoff_multiplier,
        }
    }

    /// Get the element info at the given screen coordinates.
    pub fn inspect_element(
        &self,
        x: i32,
        y: i32,
    ) -> anyhow::Result<Option<crate::core::events::ElementInfo>> {
        self.locator.inspect_at(x, y)
    }

    /// Get a clone of the replay stop flag for external monitoring.
    #[allow(dead_code)]
    pub fn get_stop_flag(&self) -> Arc<AtomicBool> {
        self.replay_stop_flag.clone()
    }

    /// Save workflow to a JSON file in the app's data directory.
    pub fn save_workflow(&self, name: &str, events: &[InputEvent]) -> anyhow::Result<PathBuf> {
        use std::fs;

        // Get the data directory
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;

        let file_path = workflow_file_path(&workflows_dir, name)?;
        let json = serde_json::to_string_pretty(events)?;
        // Encrypted at rest when a local password is configured (auth.rs).
        // Atomic write so a crash mid-save can't truncate the encrypted file.
        crate::core::security::atomic_write(&file_path, self.auth.protect(&json)?.as_bytes())?;

        Ok(file_path)
    }

    /// Load workflow from a JSON file in the app's data directory.
    pub fn load_workflow(&self, name: &str) -> anyhow::Result<Vec<InputEvent>> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        let file_path = workflow_file_path(&workflows_dir, name)?;
        // Transparently decrypts envelopes; pre-password plaintext loads as-is.
        let json = self.auth.reveal(&fs::read_to_string(&file_path)?)?;
        let events: Vec<InputEvent> = serde_json::from_str(&json)?;

        Ok(events)
    }

    /// Delete a workflow from disk.
    pub fn delete_workflow(&self, name: &str) -> anyhow::Result<()> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        let file_path = workflow_file_path(&workflows_dir, name)?;

        if file_path.exists() {
            fs::remove_file(file_path)?;
        }

        Ok(())
    }

    /// List all saved workflows with parallel directory scanning.
    pub fn list_workflows() -> anyhow::Result<Vec<String>> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");

        if !workflows_dir.exists() {
            return Ok(Vec::new());
        }

        // Use jwalk for parallel directory traversal to speed up listing
        let mut workflows: Vec<String> = jwalk::WalkDir::new(&workflows_dir)
            .into_iter()
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .collect();
        
        // Sort for deterministic ordering
        workflows.sort();

        Ok(workflows)
    }

    /// Analyze the current workflow and return AI-powered insights
    pub fn analyze_workflow(&self, events: &[InputEvent], name: &str) -> WorkflowAnalysis {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metadata = WorkflowMetadata {
            name: name.to_string(),
            description: String::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            estimated_duration_ms: events
                .iter()
                .filter_map(|e| {
                    if let InputEvent::Delay { ms, .. } = e {
                        Some(*ms)
                    } else {
                        None
                    }
                })
                .sum(),
            reliability_score: 1.0,
            element_confidence: 1.0,
        };

        self.analyzer.analyze(events, &metadata)
    }

    /// Generate a workflow object with metadata
    #[allow(dead_code)]
    pub fn create_workflow(&self, name: &str, events: &[InputEvent]) -> Workflow {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Workflow {
            name: name.to_string(),
            events: events.to_vec(),
            metadata: WorkflowMetadata {
                name: name.to_string(),
                description: format!(
                    "Automatically generated workflow with {} events",
                    events.len()
                ),
                tags: Vec::new(),
                created_at: now,
                updated_at: now,
                estimated_duration_ms: events
                    .iter()
                    .filter_map(|e| {
                        if let InputEvent::Delay { ms, .. } = e {
                            Some(*ms)
                        } else {
                            None
                        }
                    })
                    .sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
            reliability: None,
        }
    }

    /// Save a complete workflow with metadata
    pub fn save_workflow_with_metadata(&self, workflow: &Workflow) -> anyhow::Result<PathBuf> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;

        let file_path = workflow_file_path(&workflows_dir, &workflow.name)?;
        let json = serde_json::to_string_pretty(workflow)?;
        crate::core::security::atomic_write(&file_path, self.auth.protect(&json)?.as_bytes())?;

        Ok(file_path)
    }

    /// Save a workflow with custom description and tags
    #[allow(dead_code)]
    pub fn save_workflow_with_details(
        &self,
        name: &str,
        events: &[InputEvent],
        description: &str,
        tags: &[String],
    ) -> anyhow::Result<PathBuf> {
        let workflow = self.create_workflow_with_details(name, events, description, tags);
        self.save_workflow_with_metadata(&workflow)
    }

    /// Create a workflow with custom metadata
    pub fn create_workflow_with_details(
        &self,
        name: &str,
        events: &[InputEvent],
        description: &str,
        tags: &[String],
    ) -> Workflow {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Workflow {
            name: name.to_string(),
            events: events.to_vec(),
            metadata: WorkflowMetadata {
                name: name.to_string(),
                description: description.to_string(),
                tags: tags.to_vec(),
                created_at: now,
                updated_at: now,
                estimated_duration_ms: events
                    .iter()
                    .filter_map(|e| {
                        if let InputEvent::Delay { ms, .. } = e {
                            Some(*ms)
                        } else {
                            None
                        }
                    })
                    .sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
            reliability: None,
        }
    }

    /// Generate a workflow name suggestion based on the events
    pub fn generate_workflow_name(&self, events: &[InputEvent]) -> anyhow::Result<String> {
        Ok(self.analyzer.generate_workflow_name(events))
    }

    /// Load a complete workflow with metadata
    pub fn load_workflow_with_metadata(&self, name: &str) -> anyhow::Result<Workflow> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        let file_path = workflow_file_path(&workflows_dir, name)?;
        let json = self.auth.reveal(&fs::read_to_string(&file_path)?)?;
        let workflow: Workflow = serde_json::from_str(&json)?;

        Ok(workflow)
    }

    /// Replay a workflow with reliability features
    pub fn replay_with_reliability(
        &self,
        events: &[InputEvent],
        reliability: &crate::core::events::ReliabilitySettings,
        workflow_name: Option<String>,
    ) -> anyhow::Result<()> {
        // Reset flags
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);
        self.replay_progress.begin(events.len());
        *self.last_failed_step.lock().unwrap() = None;

        let started = Instant::now();
        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        let result = self.replayer.execute_with_reliability(
            events,
            self.replay_stop_flag.clone(),
            self.replay_paused.clone(),
            self.get_playback_speed(),
            self.replay_progress.clone(),
            reliability,
        );
        drop(_active);
        self.record_failed_step(&result);
        self.record_replay(workflow_name, events.len(), started, &result);

        result
    }

    /// Get element info at coordinates for validation
    #[allow(dead_code)]
    pub fn validate_element_at(&self, x: i32, y: i32) -> anyhow::Result<bool> {
        Ok(self.locator.inspect_at(x, y)?.is_some())
    }

    /// Check if replay is currently running
    pub fn is_replay_running(&self) -> bool {
        self.replay_active.load(Ordering::Relaxed)
    }

    /// Generate workflow from natural language prompt using LLM
    pub fn generate_workflow_from_prompt(
        &self,
        prompt: String,
        screenshot: Option<Vec<u8>>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        // Initialize the LLM from the persisted config if not already done
        // (it normally is, from `new()`/`update_config`).
        if llm::get_llm().is_none() {
            let ai = self.config.lock().unwrap().ai.clone();
            llm::init_llm(&LLMConfig::from_ghost_config(&ai));
        }

        let provider =
            llm::get_llm().ok_or_else(|| anyhow::anyhow!("No LLM provider available"))?;

        // Get element context from current screen
        let element_context = self.get_visible_elements()?;

        // Call the LLM (async, but we'll block on it for Tauri command)
        let rt = tokio::runtime::Runtime::new()?;
        let events = rt.block_on(async {
            provider
                .generate_workflow(
                    &prompt,
                    screenshot.as_deref(),
                    None, // AX tree would be populated here
                    &element_context,
                )
                .await
        })?;

        Ok(events)
    }

    /// Get visible elements for context
    fn get_visible_elements(&self) -> anyhow::Result<Vec<ElementInfo>> {
        let mut elements = Vec::new();

        // Probe a coarse grid — elements are tens of pixels wide, so a 48px
        // stride captures them with ~700 lookups instead of the 250k a
        // per-pixel scan would take (minutes of AX traffic per call). The grid
        // is bounded by the real display size so larger screens are covered.
        const STRIDE: usize = 48;
        let (max_x, max_y) = crate::core::vision::display_bounds();
        for y in (0..max_y).step_by(STRIDE) {
            for x in (0..max_x).step_by(STRIDE) {
                if let Ok(Some(el)) = self.locator.inspect_at(x, y) {
                    // Avoid duplicates
                    if !elements
                        .iter()
                        .any(|e: &ElementInfo| e.name == el.name && e.role == el.role)
                    {
                        elements.push(el);
                    }
                }
            }
        }

        Ok(elements)
    }

    /// Analyze and add semantic tags to recorded events
    pub fn analyze_and_tag_workflow(
        &self,
        events: Vec<InputEvent>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        if llm::get_llm().is_none() {
            let ai = self.config.lock().unwrap().ai.clone();
            llm::init_llm(&LLMConfig::from_ghost_config(&ai));
        }

        let _provider =
            llm::get_llm().ok_or_else(|| anyhow::anyhow!("No AI provider available"))?;

        let element_context = self.get_visible_elements()?;

        let rt = tokio::runtime::Runtime::new()?;
        let tagged_events = rt.block_on(async {
            // Use the analyzer for simpler heuristic-based tagging
            // LLM-based tagging would involve sending the full event stream
            let metadata = WorkflowMetadata::default();
            let _analysis = self.analyzer.analyze(&events, &metadata);

            // For each event, add semantic context
            let mut result = Vec::new();
            for event in events {
                let tagged = self.add_semantic_context(&event, &element_context);
                result.push(tagged);
            }
            result
        });

        Ok(tagged_events)
    }

    /// Add semantic context to an event. Preserves the recorded timestamp —
    /// replay pacing depends on it surviving the tagging pass.
    fn add_semantic_context(&self, event: &InputEvent, elements: &[ElementInfo]) -> InputEvent {
        match event {
            InputEvent::MouseClick {
                x,
                y,
                button,
                element,
                timestamp,
                ..
            } => {
                let semantic_tag = element
                    .clone()
                    .or_else(|| self.find_closest_element(*x, *y, elements))
                    .map(|el| crate::core::events::SemanticTag {
                        action: "click".to_string(),
                        target: el.name.clone(),
                        confidence: 0.95,
                        ui_element: Some(el.clone()),
                        ai_generated: false,
                    });

                InputEvent::MouseClick {
                    x: *x,
                    y: *y,
                    button: *button,
                    element: element.clone(),
                    timestamp: *timestamp,
                    retry_count: None,
                    semantic_tag,
                    self_heal: Some(true),
                }
            }
            InputEvent::Key {
                code,
                chars,
                modifiers,
                action,
                timestamp,
                ..
            } => {
                let semantic_tag = if !chars.is_empty() {
                    Some(crate::core::events::SemanticTag {
                        action: "type".to_string(),
                        target: format!("Keyboard input: {}", chars),
                        confidence: 0.9,
                        ui_element: None,
                        ai_generated: false,
                    })
                } else {
                    None
                };

                InputEvent::Key {
                    code: *code,
                    chars: chars.clone(),
                    modifiers: *modifiers,
                    action: action.clone(),
                    timestamp: *timestamp,
                    retry_count: None,
                    semantic_tag,
                }
            }
            other => other.clone(),
        }
    }

    /// Find the closest element to given coordinates
    fn find_closest_element(
        &self,
        x: i32,
        y: i32,
        elements: &[ElementInfo],
    ) -> Option<ElementInfo> {
        elements
            .iter()
            .filter_map(|el| {
                el.fallback_coords.as_ref().map(|(ex, ey)| {
                    // Cast to i64 before squaring: on a large virtual desktop
                    // the squared pixel delta can exceed i32::MAX and overflow.
                    let dx = (x - ex) as i64;
                    let dy = (y - ey) as i64;
                    let dist = (dx * dx + dy * dy) as f32;
                    (el, dist)
                })
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(el, _)| el.clone())
    }

    /// Wait for a condition during workflow execution
    #[allow(dead_code)]
    pub fn wait_for_condition(
        &self,
        condition: &WaitCondition,
        timeout_ms: u64,
        poll_interval_ms: u64,
    ) -> anyhow::Result<()> {
        smart_wait(
            condition,
            self.locator.as_ref(),
            timeout_ms,
            poll_interval_ms,
        )
        .map_err(|e| anyhow::anyhow!("Wait failed: {}", e))
    }

    /// Perform visual regression check
    #[allow(dead_code)]
    pub fn check_visual_regression(
        &self,
        baseline_path: &str,
        current: &DynamicImage,
        threshold: f32,
    ) -> anyhow::Result<bool> {
        let similarity = vision::compare_images(baseline_path, current)?;
        Ok(similarity >= threshold)
    }

    /// Save a screenshot to disk
    pub fn save_screenshot(&self, img: &[u8], path: &str) -> anyhow::Result<()> {
        let dynamic_image = image::load_from_memory(img)?;
        vision::save_image(&dynamic_image, path)?;
        Ok(())
    }

    // ===== Phase 4A: Visual Regression Replay =====

    /// Replay with visual regression checkpoints
    pub fn replay_with_visual_check(
        &self,
        events: &[InputEvent],
        visual_checkpoints: &[VisualCheckPoint],
    ) -> anyhow::Result<bool> {
        // Reset flags
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);
        self.replay_progress.begin(events.len());
        *self.last_failed_step.lock().unwrap() = None;

        use crate::core::replay_support::{check_continue, interruptible_sleep, pacing_gap_ms};

        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = (*self.playback_speed.lock().unwrap()).max(0.1);
        let mut prev_ts: Option<u64> = None;

        for (idx, event) in events.iter().enumerate() {
            self.replay_progress.set_step(idx);
            if !check_continue(&self.replay_stop_flag, &self.replay_paused) {
                return Ok(false);
            }

            // Reproduce the recorded rhythm between events.
            let gap = pacing_gap_ms(prev_ts, event.timestamp());
            if gap > 0
                && !interruptible_sleep(
                    (gap as f32 / speed) as u64,
                    &self.replay_stop_flag,
                    &self.replay_paused,
                )
            {
                return Ok(false);
            }
            if let Some(ts) = event.timestamp() {
                prev_ts = Some(ts);
            }

            // Check if we need to perform a visual check at this index
            let checkpoint = visual_checkpoints.iter().find(|c| c.event_index == idx);

            // Execute the event
            match event {
                InputEvent::MouseClick { x, y, button, .. } => {
                    // Mirror recorded press/release (0/2 = down, 1/3 = up) so
                    // clicks don't double-fire and drags survive.
                    let (mouse_button, direction) = match button {
                        0 => (Button::Left, Direction::Press),
                        1 => (Button::Left, Direction::Release),
                        2 => (Button::Right, Direction::Press),
                        3 => (Button::Right, Direction::Release),
                        _ => (Button::Left, Direction::Click),
                    };
                    enigo.move_mouse(*x, *y, Coordinate::Abs)?;
                    enigo.button(mouse_button, direction)?;
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    ..
                } => {
                    let key = if !chars.is_empty() {
                        Key::Unicode(chars.chars().next().unwrap_or(' '))
                    } else {
                        Key::Other(*code as u32)
                    };
                    match action {
                        KeyAction::Down => {
                            enigo.key(key, Direction::Press)?;
                        }
                        KeyAction::Up => {
                            enigo.key(key, Direction::Release)?;
                        }
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    if *dx != 0 {
                        enigo.scroll(*dx, Axis::Horizontal)?;
                    }
                    if *dy != 0 {
                        enigo.scroll(*dy, Axis::Vertical)?;
                    }
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    if !interruptible_sleep(
                        adjusted_ms,
                        &self.replay_stop_flag,
                        &self.replay_paused,
                    ) {
                        return Ok(false);
                    }
                }
                _ => {}
            }

            // Perform visual check if configured
            if let Some(checkpoint) = checkpoint {
                if let Some(baseline_path) = &checkpoint.baseline_screenshot_path {
                    if let Ok(img_bytes) = vision::capture_screenshot() {
                        if let Ok(current_img) = image::load_from_memory(&img_bytes) {
                            let similarity =
                                vision::compare_images(baseline_path, &current_img).unwrap_or(1.0);

                            if similarity < checkpoint.threshold {
                                tracing::warn!(
                                    "Visual check '{}' failed: {:.2} < {}",
                                    checkpoint.name,
                                    similarity,
                                    checkpoint.threshold
                                );
                                // Continue anyway - could be made configurable
                            } else {
                                tracing::info!(
                                    "Visual check '{}' passed: {:.2}",
                                    checkpoint.name,
                                    similarity
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(true)
    }

    /// Capture and save a baseline screenshot
    pub fn capture_baseline(
        &self,
        name: &str,
        _region: Option<(i32, i32, i32, i32)>,
    ) -> anyhow::Result<String> {
        let img_bytes = vision::capture_screenshot()
            .map_err(|e| anyhow::anyhow!("Failed to capture screenshot: {}", e))?;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let baselines_dir = data_dir.join("ghost").join("baselines");
        std::fs::create_dir_all(&baselines_dir)?;

        let path = baselines_dir.join(format!("{}.png", name));
        self.save_screenshot(&img_bytes, path.to_string_lossy().as_ref())?;

        Ok(path.to_string_lossy().to_string())
    }

    // ===== Phase 4C: Data Source Management =====

    /// Create a data source for variable-driven workflows
    pub fn create_data_source(
        &self,
        name: &str,
        source_type: &str,
        path: Option<&str>,
    ) -> anyhow::Result<String> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let sources_dir = data_dir.join("ghost").join("data_sources");
        std::fs::create_dir_all(&sources_dir)?;

        let source_path = match source_type {
            "csv" | "json" => {
                let p = path.ok_or_else(|| {
                    anyhow::anyhow!("Path required for {} data source", source_type)
                })?;
                format!("{}:{}", source_type, p)
            }
            "environment" => "environment".to_string(),
            _ => return Err(anyhow::anyhow!("Unknown source type: {}", source_type)),
        };

        let file_path = sources_dir.join(format!("{}.json", name));
        let metadata = serde_json::json!({
            "name": name,
            "type": source_type,
            "path": source_path,
            "created_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        crate::core::security::atomic_write(
            &file_path,
            serde_json::to_string_pretty(&metadata)?.as_bytes(),
        )?;
        Ok(file_path.to_string_lossy().to_string())
    }

    /// Load variables from a data source
    pub fn load_variables(
        &self,
        data_source_name: &str,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let sources_dir = data_dir.join("ghost").join("data_sources");
        let file_path = sources_dir.join(format!("{}.json", data_source_name));

        let json = std::fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read data source: {}", e))?;

        let metadata: serde_json::Value = serde_json::from_str(&json)?;
        let source_type = metadata["type"].as_str().unwrap_or("unknown");

        let mut variables = std::collections::HashMap::new();

        match source_type {
            "csv" => {
                let path = metadata["path"]
                    .as_str()
                    .and_then(|p| p.strip_prefix("csv:"))
                    .ok_or_else(|| anyhow::anyhow!("Invalid CSV path in data source"))?;

                let csv_content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read CSV file: {}", e))?;

                // Parse CSV and extract first row as variables
                for line in csv_content.lines() {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 2 {
                        variables.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
            }
            "json" => {
                let path = metadata["path"]
                    .as_str()
                    .and_then(|p| p.strip_prefix("json:"))
                    .ok_or_else(|| anyhow::anyhow!("Invalid JSON path in data source"))?;

                let json_content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read JSON file: {}", e))?;

                let json_vars: serde_json::Value = serde_json::from_str(&json_content)?;
                if let Some(obj) = json_vars.as_object() {
                    for (k, v) in obj {
                        variables.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                    }
                }
            }
            "environment" => {
                // Load from environment variables
                for (key, value) in std::env::vars() {
                    variables.insert(key, value);
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown source type: {}", source_type)),
        }

        Ok(variables)
    }

    // ===== Smart Observer Mode Methods =====

    /// Start the Smart Observer - watch and learn user patterns
    pub fn start_observer(&self) {
        self.knowledge_base.start_observer();
    }

    /// Stop the Smart Observer
    pub fn stop_observer(&self) {
        self.knowledge_base.stop_observer();
    }

    /// Check if observer is active
    pub fn is_observer_active(&self) -> bool {
        self.knowledge_base.is_observer_active()
    }

    /// Set observer interval in milliseconds
    pub fn set_observer_interval(&self, interval_ms: u64) {
        self.knowledge_base.set_observer_interval(interval_ms);
    }

    /// Record events as an observed pattern
    pub fn observe_events(&self, events: &[InputEvent], app_name: &str) {
        let patterns = self
            .knowledge_base
            .analyze_observed_events(events, app_name);
        for pattern in patterns {
            self.knowledge_base.observe_pattern(pattern);
        }
        self.knowledge_base.track_app_usage(app_name);
    }

    /// Get proactive automation suggestions
    pub fn get_proactive_suggestions(&self) -> Vec<ProactiveSuggestion> {
        self.knowledge_base.get_suggestions()
    }

    /// Get learned patterns for an app
    pub fn get_learned_patterns(&self, app_name: Option<&str>) -> Vec<LearnedPattern> {
        match app_name {
            Some(name) => self.knowledge_base.get_app_patterns(name),
            None => self.knowledge_base.get_patterns(),
        }
    }

    /// Get app usage statistics
    pub fn get_app_usage_stats(&self) -> Vec<crate::core::knowledge::AppUsageStats> {
        self.knowledge_base.get_app_usage()
    }

    /// Get execution tracker reference
    pub fn get_execution_tracker(
        &self,
    ) -> Option<std::sync::MutexGuard<'_, Option<ExecutionHistory>>> {
        self.execution_tracker.lock().ok()
    }

    /// Generate a "geek mode" insight for events
    pub fn generate_geek_insights(
        &self,
        events: &[InputEvent],
        _app_name: &str,
    ) -> crate::core::knowledge::GeekDetails {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let timings: Vec<_> = events
            .iter()
            .enumerate()
            .map(|(idx, _)| crate::core::knowledge::EventTiming {
                event_index: idx,
                timestamp_ms: now,
                delay_before_ms: 0,
                estimated_action: "pending analysis".to_string(),
            })
            .collect();

        let total_ms: u64 = events
            .iter()
            .filter_map(|e| match e {
                InputEvent::Delay { ms, .. } => Some(*ms),
                _ => None,
            })
            .sum();

        crate::core::knowledge::GeekDetails {
            event_timing_analysis: timings,
            system_calls_traced: vec!["mouse_event".to_string(), "key_event".to_string()],
            alternative_shortcuts: vec![],
            performance_metrics: crate::core::knowledge::PerformanceMetrics {
                total_duration_ms: total_ms,
                avg_delay_ms: total_ms as f64 / events.len().max(1) as f64,
                bottleneck_events: vec![],
            },
            raw_ax_tree_snapshots: vec![],
        }
    }
}

impl Default for GhostEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_file_path_adds_extension_for_safe_names() {
        let base = std::env::temp_dir().join("ghost_engine_workflow_path");
        let path = workflow_file_path(&base, "Daily Report_1").unwrap();

        assert_eq!(path, base.join("Daily Report_1.json"));
    }

    #[test]
    fn workflow_file_path_rejects_traversal_names() {
        let base = std::env::temp_dir().join("ghost_engine_workflow_path");

        assert!(workflow_file_path(&base, "../secrets").is_err());
        assert!(workflow_file_path(&base, "nested/workflow").is_err());
        assert!(workflow_file_path(&base, "workflow.json").is_err());
    }

    #[test]
    fn buffer_event_leaves_template_png_unset_when_capture_disabled() {
        // capture_element_templates is off by default — buffer_event must
        // not attempt (or need) a screenshot at all in that case, so this is
        // deterministic across every CI platform regardless of display
        // availability.
        let engine = GhostEngine::new();
        let click = InputEvent::MouseClick {
            x: 10,
            y: 10,
            button: 0,
            element: Some(ElementInfo {
                role: "AXButton".into(),
                name: "Save".into(),
                app: "Notes".into(),
                ..Default::default()
            }),
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        };
        engine.buffer_event(click);

        let recorded = engine.get_recorded_events();
        match &recorded[0] {
            InputEvent::MouseClick { element, .. } => {
                assert!(element.as_ref().unwrap().template_png.is_none());
            }
            other => panic!("expected MouseClick, got {:?}", other),
        }
    }

    #[test]
    fn telemetry_off_by_default_collects_nothing() {
        let engine = GhostEngine::new();
        // A fresh engine inherits the persisted privacy flag (false by default),
        // so feature tracking must be a no-op until explicitly opted in.
        engine.track_feature("analyze_workflow");
        let stats = engine.get_telemetry_stats();
        assert!(stats.feature_usage.is_empty());
    }

    #[test]
    fn enabling_telemetry_records_feature_usage() {
        let engine = GhostEngine::new();
        engine.telemetry.set_enabled(true);

        engine.track_feature("analyze_workflow");
        engine.track_feature("analyze_workflow");
        engine.track_feature("optimize_workflow");

        let stats = engine.get_telemetry_stats();
        assert_eq!(stats.feature_usage.get("analyze_workflow"), Some(&2));
        assert_eq!(stats.feature_usage.get("optimize_workflow"), Some(&1));

        // Export should round-trip the collected data as JSON.
        let json = engine.export_telemetry().expect("export should succeed");
        assert!(json.contains("analyze_workflow"));
    }

    #[test]
    fn set_playback_speed_clamps_and_round_trips() {
        let engine = GhostEngine::new();

        // A normal factor passes through unchanged so the speed picker actually
        // takes effect (it once silently did nothing).
        engine.set_playback_speed(2.5);
        assert_eq!(engine.get_playback_speed(), 2.5);

        // Non-positive / tiny factors clamp to the 0.1 floor instead of
        // freezing or reversing replay.
        engine.set_playback_speed(0.0);
        assert_eq!(engine.get_playback_speed(), 0.1);
        engine.set_playback_speed(-5.0);
        assert_eq!(engine.get_playback_speed(), 0.1);
    }

    #[test]
    fn is_replay_running_false_when_idle() {
        // Reflects the real `replay_active` flag, not the stop flag, so a fresh
        // engine reports no replay in flight.
        let engine = GhostEngine::new();
        assert!(!engine.is_replay_running());
    }

    #[test]
    fn update_config_live_applies_playback_speed() {
        let engine = GhostEngine::new();
        // Snapshot the persisted config so this test can restore it and not
        // leak a non-default speed into other tests sharing the on-disk config.
        let original = GhostConfig::load().unwrap_or_default();

        let mut updated = original.clone();
        updated.replay.default_speed = 3.0;
        engine.update_config(updated).expect("valid config applies");
        assert_eq!(engine.get_playback_speed(), 3.0);

        // Restore the original persisted config.
        engine.update_config(original).expect("restore config");
    }

    #[test]
    fn start_recording_rejects_reentrant_session() {
        let engine = GhostEngine::new();

        // Simulate an already-active recording by populating the sender slot.
        let (active_tx, _active_rx) = mpsc::channel();
        *engine.tx.lock().unwrap() = Some(active_tx);

        let (tx, _rx) = mpsc::channel();
        let result = engine.start_recording(tx);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Recording already active"));
    }
}
