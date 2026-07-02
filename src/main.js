// Ghost desktop app — Tauri IPC integration, recording controls, workflow
// management, and Smart Observer. This is the real app UI
// (not the marketing site — that lives in public/).

const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};

function notAvailable() {
  toastError("Tauri not available — running in static mode");
}

// Recording state
let isRecording = false;
let recordedEvents = [];
let isPlaying = false;
let isPaused = false;
let playbackSpeed = 1.0;
let guardAuditCompleted = false;
let hasReplayedCurrentWorkflow = false;
let hasSavedCurrentWorkflow = false;
let latestGuardReport = null;
const MAX_TIMELINE_ITEMS = 220;
const pendingTimelineEvents = [];
let timelineFlushScheduled = false;

// Listen for ghost events from the backend
if (listen) {
  listen("ghost:event", (event) => {
    console.log("Ghost event captured:", event.payload);
    if (isRecording) {
      recordedEvents.push(event.payload);
      updateRecordingUI();
      queueTimelineEvent(event.payload);
    }
  });

  listen("ghost:guard", (event) => {
    showNotification(String(event.payload || "Ghost Guard suppressed sensitive input."), "error");
    showInsight("Ghost Guard is protecting sensitive input. Stop recording before typing secrets.");
  });
}

function showInsight(text) {
  const el = document.getElementById("insight-text");
  if (el) el.textContent = text;
}

function showNotification(text, kind = "info") {
  const notificationsEl = document.getElementById("notifications");
  if (!notificationsEl) return;

  const notification = document.createElement("div");
  notification.className = `notification notification--${kind}`;
  const icon = kind === "error" ? "⚠️" : "✓";
  notification.innerHTML = `<p class="notification__text">${icon} ${escapeHtml(text)}</p>`;
  notificationsEl.appendChild(notification);

  setTimeout(() => notification.remove(), kind === "error" ? 8000 : 5000);
}

const toastError = (text) => showNotification(text, "error");

function escapeHtml(value) {
  const div = document.createElement("div");
  div.textContent = String(value ?? "");
  return div.innerHTML;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ===== In-app dialogs (replace browser prompt()/alert(), which look
// terrible inside a desktop app) =====

function ghostPrompt(message, defaultValue = "", placeholder = "") {
  return new Promise((resolve) => {
    const modal = document.getElementById("input-modal");
    const content = modal?.querySelector(".modal-content");
    if (!content) return resolve(window.prompt(message, defaultValue)); // fallback

    content.innerHTML = `
      <h3 style="margin-top:0">${escapeHtml(message)}</h3>
      <input type="text" data-dialog-input placeholder="${escapeHtml(placeholder)}"
             style="width:100%;margin:8px 0 16px;padding:8px 10px;background:var(--bg);border:1px solid var(--border);border-radius:8px;color:var(--text);font-size:0.95rem;">
      <div style="display:flex;gap:8px;justify-content:flex-end;">
        <button class="btn btn--ghost btn--small" data-dialog-cancel>Cancel</button>
        <button class="btn btn--primary btn--small" data-dialog-ok>OK</button>
      </div>`;
    modal.style.display = "flex";

    const input = content.querySelector("[data-dialog-input]");
    input.value = defaultValue ?? "";
    const done = (val) => {
      modal.style.display = "none";
      resolve(val);
    };
    content.querySelector("[data-dialog-ok]").addEventListener("click", () => done(input.value));
    content.querySelector("[data-dialog-cancel]").addEventListener("click", () => done(null));
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") done(input.value);
      if (e.key === "Escape") done(null);
    });
    input.focus();
    input.select();
  });
}

function ghostPick(message, options) {
  return new Promise((resolve) => {
    const modal = document.getElementById("input-modal");
    const content = modal?.querySelector(".modal-content");
    if (!content) return resolve(window.prompt(message));

    content.innerHTML = `
      <h3 style="margin-top:0">${escapeHtml(message)}</h3>
      <div style="display:flex;flex-direction:column;gap:6px;margin:8px 0 16px;max-height:50vh;overflow-y:auto;">
        ${options.length === 0 ? '<p style="color:var(--muted)">Nothing here yet.</p>' : ""}
        ${options.map((o) => `<button class="btn btn--ghost" data-dialog-option="${escapeAttr(o)}" style="justify-content:flex-start;text-align:left;">${escapeHtml(o)}</button>`).join("")}
      </div>
      <div style="display:flex;justify-content:flex-end;">
        <button class="btn btn--ghost btn--small" data-dialog-cancel>Cancel</button>
      </div>`;
    modal.style.display = "flex";

    const done = (val) => {
      modal.style.display = "none";
      resolve(val);
    };
    content.querySelectorAll("[data-dialog-option]").forEach((btn) =>
      btn.addEventListener("click", () => done(btn.dataset.dialogOption)),
    );
    content.querySelector("[data-dialog-cancel]").addEventListener("click", () => done(null));
  });
}

// A yes/no confirmation built on the same in-app dialog. Resolves true only on
// explicit approval — used to gate Organizer execution behind a clear consent.
async function ghostConfirm(message, confirmLabel = "Yes") {
  const choice = await ghostPick(message, [confirmLabel]);
  return choice === confirmLabel;
}

// ===== Accessibility permission gate =====

// Recording needs BOTH macOS permissions: Accessibility (clicks) and
// Input Monitoring (keystrokes). Missing either means the event tap only
// receives scroll events.
async function checkPermissions() {
  const [accessibility, inputMonitoring] = await Promise.all([
    invoke("check_accessibility"),
    invoke("check_input_monitoring").catch(() => true), // older backends
  ]);
  return { accessibility, inputMonitoring };
}

async function refreshPermissionBanner() {
  if (!invoke) return;

  const banner = document.getElementById("perm-banner");
  if (!banner) return;

  try {
    const { accessibility, inputMonitoring } = await checkPermissions();
    banner.hidden = accessibility && inputMonitoring;
    updateMissionProgress({ permissionsGranted: accessibility && inputMonitoring });

    const text = document.getElementById("perm-text");
    if (text && !banner.hidden) {
      const missing = [];
      if (!accessibility) missing.push("Accessibility");
      if (!inputMonitoring) missing.push("Input Monitoring");
      text.textContent = `Ghost needs ${missing.join(" and ")} permission to record clicks and keystrokes.`;
    }
  } catch (error) {
    console.error("Failed to check permissions:", error);
  }
}

async function requestAccessibility() {
  if (!invoke) return;
  try {
    const { accessibility, inputMonitoring } = await checkPermissions();
    // macOS shows each permission prompt only once per app; afterwards the
    // backend opens the matching System Settings pane instead.
    if (!accessibility) await invoke("request_accessibility");
    if (!inputMonitoring) await invoke("request_input_monitoring").catch(() => {});

    const after = await checkPermissions();
    if (!after.accessibility || !after.inputMonitoring) {
      showNotification(
        "Enable Ghost in System Settings → Privacy & Security (Accessibility + Input Monitoring), then quit and reopen Ghost.",
      );
    }
  } catch (error) {
    console.error("Failed to request permissions:", error);
  } finally {
    refreshPermissionBanner();
  }
}

// ===== Local login (lock screen + at-rest encryption) =====

// Mirrors the backend auth_status command. When no password is configured
// the app behaves exactly as before; `unlocked` only matters if `configured`.
let authStatus = { configured: false, unlocked: true };

async function refreshAuthStatus() {
  if (!invoke) return;
  try {
    authStatus = await invoke("auth_status");
  } catch (error) {
    console.error("Failed to fetch auth status:", error);
  }
  const lockBtn = document.getElementById("lockBtn");
  if (lockBtn) lockBtn.hidden = !authStatus.configured;
}

function showLockScreen() {
  const overlay = document.getElementById("lock-screen");
  if (!overlay) return;
  overlay.hidden = false;
  const input = document.getElementById("lockPassword");
  if (input) {
    input.value = "";
    input.focus();
  }
}

async function tryUnlock() {
  if (!invoke) return;
  const input = document.getElementById("lockPassword");
  const error = document.getElementById("lockError");
  const password = input?.value ?? "";

  try {
    const ok = await invoke("auth_unlock", { password });
    if (!ok) {
      if (error) error.hidden = false;
      if (input) {
        input.value = "";
        input.focus();
      }
      return;
    }
  } catch (err) {
    console.error("Unlock failed:", err);
    toastError("Unlock failed: " + err);
    return;
  }

  if (error) error.hidden = true;
  const overlay = document.getElementById("lock-screen");
  if (overlay) overlay.hidden = true;
  await refreshAuthStatus();
  showInsight("Unlocked. Your workflows are ready.");
  maybeShowOnboarding();
}

async function lockApp() {
  if (!invoke) return;
  try {
    await invoke("auth_lock");
  } catch (error) {
    console.error("Failed to lock:", error);
    return;
  }
  await refreshAuthStatus();
  showLockScreen();
}

// Decides what greets the user on launch: the lock screen when a password is
// configured and the app is locked, otherwise the first-run walkthrough.
async function initAuthGate() {
  await refreshAuthStatus();
  if (authStatus.configured && !authStatus.unlocked) {
    showLockScreen();
    return;
  }
  maybeShowOnboarding();
}

// ===== First-run onboarding =====

const ONBOARDING_KEY = "ghost.onboarding.completed";
const ONBOARDING_PERM_STEP = 2; // index of the permissions step (needs polling)
const ONBOARDING_PASSWORD_STEP = 3;
let onboardingStep = 0;
let permPollTimer = null;

function maybeShowOnboarding() {
  let done = false;
  try {
    done = localStorage.getItem(ONBOARDING_KEY) === "1";
  } catch (_) {
    // localStorage unavailable (e.g. static mode) — show onboarding anyway
  }
  if (done) return;

  const overlay = document.getElementById("onboarding");
  if (!overlay) return;
  overlay.hidden = false;
  showOnboardingStep(0);
}

function showOnboardingStep(n) {
  onboardingStep = n;

  document.querySelectorAll(".onboarding__step").forEach((el) => {
    el.hidden = Number(el.dataset.step) !== n;
  });
  document.querySelectorAll(".onboarding__dot").forEach((dot) => {
    dot.classList.toggle("is-active", Number(dot.dataset.dot) === n);
  });

  // The permission step needs live status polling.
  if (n === ONBOARDING_PERM_STEP) {
    refreshOnboardingPermStatus();
    startPermPolling();
  } else {
    stopPermPolling();
  }

  // The password step is skipped entirely if a password already exists
  // (e.g. user re-runs the tour after setting one up).
  if (n === ONBOARDING_PASSWORD_STEP && authStatus.configured) {
    showOnboardingStep(n + 1);
  }
}

// Validate the password fields and create the local password via the backend.
async function onboardingSetPassword() {
  const password = document.getElementById("setupPassword")?.value ?? "";
  const confirm = document.getElementById("setupPasswordConfirm")?.value ?? "";
  const errorEl = document.getElementById("setupPasswordError");
  const fail = (msg) => {
    if (errorEl) {
      errorEl.textContent = msg;
      errorEl.hidden = false;
    }
  };

  if (password.length < 8) return fail("Password must be at least 8 characters.");
  if (password !== confirm) return fail("Passwords don't match.");
  if (!invoke) return fail("Tauri not available — running in static mode.");

  try {
    await invoke("auth_setup", { password });
  } catch (error) {
    console.error("Failed to set password:", error);
    return fail("Could not set password: " + error);
  }

  if (errorEl) errorEl.hidden = true;
  await refreshAuthStatus();
  showNotification("Password set — your workflows are now encrypted on this device.");
  showOnboardingStep(ONBOARDING_PASSWORD_STEP + 1);
}

async function refreshOnboardingPermStatus() {
  if (!invoke) return;
  let granted = false;
  try {
    const { accessibility, inputMonitoring } = await checkPermissions();
    granted = accessibility && inputMonitoring;
  } catch (error) {
    console.error("Failed to check permissions:", error);
    return;
  }

  const status = document.getElementById("onboardingPermStatus");
  const text = document.getElementById("onboardingPermStatusText");
  const next = document.getElementById("onboardingPermNext");
  const grant = document.getElementById("onboardingGrant");
  if (!status) return;

  status.dataset.granted = granted ? "true" : "false";
  if (text) text.textContent = granted ? "✓ Access granted" : "Not granted yet";

  // Once granted, make "Next" the obvious action and de-emphasize "Grant".
  if (granted) {
    stopPermPolling();
    if (next) {
      next.classList.add("btn--primary");
      next.classList.remove("btn--ghost");
    }
    if (grant) {
      grant.classList.add("btn--ghost", "btn--small");
      grant.classList.remove("btn--primary");
    }
  }
}

function startPermPolling() {
  stopPermPolling();
  permPollTimer = setInterval(refreshOnboardingPermStatus, 1500);
}

function stopPermPolling() {
  if (permPollTimer) {
    clearInterval(permPollTimer);
    permPollTimer = null;
  }
}

async function onboardingGrant() {
  await requestAccessibility();
  refreshOnboardingPermStatus();
  startPermPolling();
}

function finishOnboarding() {
  stopPermPolling();
  try {
    localStorage.setItem(ONBOARDING_KEY, "1");
  } catch (_) {
    // ignore — onboarding will simply re-show next launch
  }
  const overlay = document.getElementById("onboarding");
  if (overlay) overlay.hidden = true;
  refreshPermissionBanner();
}

// ===== Recording & replay =====

async function startRecording() {
  if (!invoke) return notAvailable();

  try {
    await invoke("start_recording");
    isRecording = true;
    recordedEvents = [];
    resetReplayInspectionState();
    guardAuditCompleted = false;
    latestGuardReport = null;
    hasReplayedCurrentWorkflow = false;
    hasSavedCurrentWorkflow = false;
    const timelineEl = document.getElementById("events-timeline");
    if (timelineEl) timelineEl.innerHTML = "";
    updateRecordingUI();
    showInsight("Recording. Keep the task focused; stop before entering sensitive information.");
  } catch (error) {
    console.error("Failed to start recording:", error);
    toastError("Could not start recording: " + error);
    showInsight("Recording blocked — check permissions above.");
  }
}

async function stopRecording() {
  if (!invoke) return notAvailable();

  try {
    await invoke("stop_recording");
    isRecording = false;
    updateRecordingUI();
    showInsight(`Captured ${recordedEvents.length} event(s). Review the timeline, then replay or save.`);
    updateWorkflowHealth();
    runGhostGuardAudit({ quiet: true });
  } catch (error) {
    console.error("Failed to stop recording:", error);
  }

  observerLearnFromSession();
}

// Most frequent app among the recorded elements — used so Smart Observer can
// file patterns without interrupting the user with a prompt.
function dominantAppName(events) {
  const counts = {};
  for (const ev of events) {
    const { data } = normalizeEvent(ev);
    const app = data?.element?.app;
    if (app && app !== "Unknown") counts[app] = (counts[app] || 0) + 1;
  }
  let best = null;
  let bestCount = 0;
  for (const [app, count] of Object.entries(counts)) {
    if (count > bestCount) {
      best = app;
      bestCount = count;
    }
  }
  return best || "Unknown App";
}

// While Smart Observer is active, every finished recording session feeds the
// knowledge base automatically — no manual "Observe Session" step needed.
async function observerLearnFromSession() {
  if (!invoke || recordedEvents.length === 0) return;

  try {
    if (!(await invoke("is_observer_active"))) return;

    const appName = dominantAppName(recordedEvents);
    const patternsFound = await invoke("observe_events", {
      events: recordedEvents,
      appName: appName,
    });

    if (patternsFound > 0) {
      showInsight(`Observer learned ${patternsFound} pattern(s) from this session.`);
      const suggestions = await invoke("get_proactive_suggestions");
      if (suggestions.length > 0) displaySuggestions(suggestions);
    }
  } catch (error) {
    console.error("Observer session learning failed:", error);
  }
}

async function confirmGuardBeforeReplay() {
  if (recordedEvents.length === 0) return false;
  if (!latestGuardReport) {
    await runGhostGuardAudit({ quiet: true });
  }

  if (latestGuardReport?.blocks_replay) {
    const steps = latestGuardReport.ai_audit?.blocked_steps?.join(", ") || "unknown";
    toastError(`Replay blocked by Ghost Guard. Remove sensitive step(s): ${steps}.`);
    showInsight("Replay blocked: Ghost Guard found stored secret-like input. Re-record with manual checkpoints.");
    return false;
  }

  if (latestGuardReport?.requires_confirmation) {
    const ok = await ghostPick("Ghost Guard found high-risk steps. Continue?", ["Review first", "Replay anyway"]);
    if (ok !== "Replay anyway") {
      showInsight("Review the timeline and Ghost Guard findings before replay.");
      return false;
    }
  }

  return true;
}

async function replayWorkflow() {
  if (!invoke) return notAvailable();
  if (!(await confirmGuardBeforeReplay())) return;

  try {
    isPlaying = true;
    lastFailedStep = null;
    updateRecordingUI();
    startReplayProgressPolling();
    await invoke("replay_workflow", { events: recordedEvents });
    hasReplayedCurrentWorkflow = true;
    await summarizeLastReplayResolution();
  } catch (error) {
    console.error("Failed to replay workflow:", error);
    toastError("Replay failed: " + error);
    await captureFailedStep(0);
  } finally {
    isPlaying = false;
    stopReplayProgressPolling();
    updateRecordingUI();
  }
}

async function replayWithReliability() {
  if (!invoke) return notAvailable();

  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  if (!(await confirmGuardBeforeReplay())) return;

  try {
    // Retry behavior comes from Settings (replay.*) — no popups.
    const config = await invoke("get_config");
    const replay = config?.replay ?? {};

    isPlaying = true;
    lastFailedStep = null;
    updateRecordingUI();
    startReplayProgressPolling();
    await invoke("replay_with_reliability", {
      events: recordedEvents,
      maxAttempts: replay.max_retry_attempts ?? 3,
      backoffMs: replay.retry_backoff_ms ?? 500,
      backoffMultiplier: replay.retry_backoff_multiplier ?? 2.0,
    });
    hasReplayedCurrentWorkflow = true;
    await summarizeLastReplayResolution();
  } catch (error) {
    console.error("Failed to replay with reliability:", error);
    toastError("Replay failed: " + error);
    await captureFailedStep(0);
  } finally {
    isPlaying = false;
    stopReplayProgressPolling();
    updateRecordingUI();
  }
}

async function cancelReplay() {
  if (!invoke) return;
  try {
    await invoke("cancel_replay");
    isPlaying = false;
    isPaused = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to cancel replay:", error);
  }
}

async function pauseReplay() {
  if (!invoke) return;
  try {
    await invoke("pause_replay");
    isPaused = true;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to pause replay:", error);
  }
}

async function resumeReplay() {
  if (!invoke) return;
  try {
    await invoke("resume_replay");
    isPaused = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to resume replay:", error);
  }
}

async function setSpeed(factor) {
  if (!invoke) return;
  try {
    await invoke("set_playback_speed", { factor });
    playbackSpeed = factor;
  } catch (error) {
    console.error("Failed to set speed:", error);
  }
}

async function inspectElementAtCursor() {
  if (!invoke) return notAvailable();

  // Give the user time to hover the element they care about.
  for (let i = 3; i > 0; i--) {
    showInsight(`Hover over any element — inspecting in ${i}…`);
    await sleep(1000);
  }

  try {
    const { x, y, element } = await invoke("inspect_element_at_cursor");
    if (element) {
      const name = element.name ? ` "${element.name}"` : "";
      const app = element.app && element.app !== "Unknown" ? ` in ${element.app}` : "";
      showInsight(`(${x}, ${y}) → ${element.role || "element"}${name}${app}`);
      showNotification(`${element.role || "element"}${name}${app}`);
    } else {
      showInsight(`No accessible element at (${x}, ${y}).`);
    }
  } catch (error) {
    console.error("Failed to inspect element:", error);
    toastError("Inspect failed: " + error);
  }
}


// ===== Ghost Guard local audit =====

function severityIcon(severity) {
  return { low: "ℹ️", medium: "⚠️", high: "🛡️", critical: "🚨" }[severity] || "•";
}

function renderGuardReport(report) {
  const card = document.getElementById("ghostGuardCard");
  const scoreEl = document.getElementById("guardScore");
  const summaryEl = document.getElementById("guardSummary");
  if (!card || !scoreEl || !summaryEl) return;

  card.dataset.risk = report.risk_level;
  scoreEl.textContent = `${report.score}/100 · ${report.risk_level.toUpperCase()} risk`;

  const topFindings = (report.findings || []).slice(0, 3);
  const findingsHtml = topFindings
    .map((finding) => `${severityIcon(finding.severity)} ${escapeHtml(finding.title)}`)
    .join("<br>");
  const nextSteps = report.ai_audit?.recommended_next_steps?.slice(0, 2) || [];
  const nextHtml = nextSteps.map((step) => `• ${escapeHtml(step)}`).join("<br>");
  summaryEl.innerHTML = `${escapeHtml(report.summary)}${findingsHtml ? `<br>${findingsHtml}` : ""}${nextHtml ? `<br>${nextHtml}` : ""}`;

  latestGuardReport = report;
  guardAuditCompleted = true;
  updateMissionProgress();

  if (report.requires_confirmation) {
    showNotification("Ghost Guard found high-risk steps. Use step-by-step review before replay.", "error");
  } else {
    showNotification("Ghost Guard audit complete.");
  }
}

async function runGhostGuardAudit({ quiet = false } = {}) {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    if (!quiet) toastError("No events to audit yet");
    return;
  }

  try {
    const report = await invoke("ghost_guard_audit", { events: recordedEvents });
    renderGuardReport(report);
    if (!quiet) showInsight(`Ghost Guard: ${report.score}/100 ${report.risk_level} risk. ${report.summary}`);
  } catch (error) {
    console.error("Ghost Guard audit failed:", error);
    if (!quiet) toastError("Ghost Guard audit failed: " + error);
  }
}

// ===== Workflow management =====

async function saveWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }
  if (!latestGuardReport) await runGhostGuardAudit({ quiet: true });
  if (latestGuardReport && latestGuardReport.safe_to_save === false) {
    toastError("Save blocked: Ghost Guard found secret-like input. Re-record without passwords, tokens, or payment data.");
    return;
  }

  const name = await ghostPrompt("Name this workflow", "", "e.g. Friday timesheet");
  if (!name) return;

  try {
    await invoke("save_workflow", { name, events: recordedEvents });
    hasSavedCurrentWorkflow = true;
    updateMissionProgress();
    showNotification(`Workflow "${name}" saved.`);
  } catch (error) {
    console.error("Failed to save workflow:", error);
    toastError("Failed to save workflow: " + error);
  }
}

async function loadWorkflow() {
  if (!invoke) return;

  try {
    const names = await invoke("list_workflows");
    const name = await ghostPick("Load a workflow", names);
    if (!name) return;

    recordedEvents = await invoke("load_workflow", { name });
    resetReplayInspectionState();
    guardAuditCompleted = false;
    latestGuardReport = null;
    hasReplayedCurrentWorkflow = false;
    hasSavedCurrentWorkflow = false;
    updateRecordingUI();
    refreshTimeline();
    showNotification(`Loaded "${name}" — ${recordedEvents.length} events.`);
    runGhostGuardAudit({ quiet: true });
  } catch (error) {
    console.error("Failed to load workflow:", error);
    toastError("Failed to load workflow: " + error);
  }
}

// ===== AI-powered workflow functions =====

async function analyzeWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    const analysis = await invoke("analyze_workflow", { name: "Current recording", events: recordedEvents });
    displayAnalysisResults(analysis);
  } catch (error) {
    console.error("Failed to analyze workflow:", error);
    toastError("Failed to analyze workflow: " + error);
  }
}

async function optimizeWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    const optimized = await invoke("optimize_workflow", { events: recordedEvents });
    const originalCount = recordedEvents.length;
    recordedEvents = optimized;
    resetReplayInspectionState();
    guardAuditCompleted = false;
    updateRecordingUI();
    refreshTimeline();
    showNotification(`Optimized: ${originalCount} events → ${optimized.length} events.`);
  } catch (error) {
    console.error("Failed to optimize workflow:", error);
    toastError("Failed to optimize workflow: " + error);
  }
}

function refreshTimeline() {
  const timelineEl = document.getElementById("events-timeline");
  if (timelineEl) {
    timelineEl.innerHTML = "";
    recordedEvents.forEach((event) => addEventToTimeline(event));
    trimTimeline();
    updateWorkflowHealth();
  }
}

// Describe a workflow in plain language and let the configured LLM build it.
// With the "local" provider this falls back to keyword heuristics, so steer
// users toward a real provider for anything non-trivial.
async function generateWorkflowFromDescription() {
  if (!invoke) return notAvailable();

  const prompt = await ghostPrompt(
    "Describe the workflow to generate",
    "",
    "e.g. click the Save button, then press Enter",
  );
  if (!prompt) return;

  let providerNote = "";
  try {
    const config = await invoke("get_config");
    if ((config?.ai?.provider ?? "local") === "local") {
      providerNote =
        " Heuristic mode: pick the openai/anthropic provider in Settings (with an API key) for real AI generation.";
    }
  } catch (_) {
    // Settings unavailable — generate anyway.
  }

  try {
    showInsight("Generating workflow from your description…");
    const events = await invoke("generate_workflow_from_prompt", { prompt });
    recordedEvents = events;
    resetReplayInspectionState();
    guardAuditCompleted = false;
    hasReplayedCurrentWorkflow = false;
    hasSavedCurrentWorkflow = false;
    updateRecordingUI();
    refreshTimeline();
    showNotification(`Generated ${events.length} events from your description.${providerNote}`);
    showInsight("Review the generated steps in the timeline, then Replay or Save.");
  } catch (error) {
    console.error("Failed to generate workflow:", error);
    toastError("Generate failed: " + error);
  }
}

async function suggestWorkflowName() {
  if (!invoke) return ghostPrompt("Name this workflow");
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    const suggestion = await invoke("suggest_workflow_name", { events: recordedEvents });
    return (await ghostPrompt("Workflow name (AI suggested)", suggestion)) || suggestion;
  } catch (error) {
    console.error("Failed to suggest name:", error);
    return ghostPrompt("Name this workflow");
  }
}

async function saveWorkflowWithMetadata() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  if (!latestGuardReport) await runGhostGuardAudit({ quiet: true });
  if (latestGuardReport && latestGuardReport.safe_to_save === false) {
    toastError("Save blocked: Ghost Guard found secret-like input. Re-record without passwords, tokens, or payment data.");
    return;
  }

  try {
    const name = await suggestWorkflowName();
    if (!name) return;

    const description = (await ghostPrompt("Short description (optional)", "")) ?? "";
    const tagsInput = (await ghostPrompt("Tags, comma-separated (optional)", "")) ?? "";
    const tags = tagsInput.split(",").map((t) => t.trim()).filter((t) => t);

    await invoke("save_workflow_with_metadata", {
      name,
      events: recordedEvents,
      description,
      tags,
    });
    hasSavedCurrentWorkflow = true;
    updateMissionProgress();
    showNotification(`Workflow "${name}" saved with metadata.`);
  } catch (error) {
    console.error("Failed to save workflow:", error);
    toastError("Failed to save workflow: " + error);
  }
}

function displayAnalysisResults(analysis) {
  const modal = document.getElementById("analysis-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>Local Workflow Analysis: ${escapeHtml(analysis.workflow_name)}</h3>
    <p class="panel__hint">This is deterministic local analysis, not a connected AI model.</p>
    <p><strong>Total Events:</strong> ${analysis.total_events}</p>
    <p><strong>Estimated Duration:</strong> ${analysis.estimated_duration_ms}ms</p>
    <p><strong>Reliability Score:</strong> ${(analysis.reliability_score * 100).toFixed(1)}%</p>
    <p><strong>Element Richness:</strong> ${(analysis.element_richness * 100).toFixed(1)}%</p>

    ${analysis.patterns.length > 0 ? `
    <h4>Detected Patterns</h4>
    <ul>
      ${analysis.patterns.map((p) => `<li>${escapeHtml(p.description)} (confidence: ${(p.confidence * 100).toFixed(1)}%)</li>`).join("")}
    </ul>
    ` : ""}

    ${analysis.suggested_optimizations.length > 0 ? `
    <h4>Suggested Optimizations</h4>
    <ul>
      ${analysis.suggested_optimizations.map((o) => `<li>${escapeHtml(o.description)}</li>`).join("")}
    </ul>
    ` : ""}

    <button data-close-modal="analysis-modal">Close</button>
  `;

  showModal(modal);
}

// Tracks the element focused before a modal opened so focus can be restored on
// close (basic accessibility: keyboard/screen-reader users land back where they
// were instead of at the top of the document).
let lastFocusedBeforeModal = null;

function showModal(modal) {
  if (!modal) return;
  lastFocusedBeforeModal = document.activeElement;
  modal.style.display = "flex";
  // Move focus into the dialog (the .modal-content carries role="dialog" and
  // tabindex="-1" so it is programmatically focusable).
  const content = modal.querySelector(".modal-content");
  if (content) content.focus();
}

function closeModal(modalId = "analysis-modal") {
  const modal = document.getElementById(modalId);
  if (!modal) return;
  modal.style.display = "none";
  // Clear the dialog body so stale markup and its event listeners don't
  // accumulate across repeated opens.
  const content = modal.querySelector(".modal-content");
  if (content) content.innerHTML = "";
  // Restore focus to wherever it was before the modal opened.
  if (lastFocusedBeforeModal && typeof lastFocusedBeforeModal.focus === "function") {
    lastFocusedBeforeModal.focus();
  }
  lastFocusedBeforeModal = null;
}

// ===== Settings =====

// Cache the full config so we can send a complete GhostConfig back to the
// backend (update_config deserializes the whole struct, not a partial patch).
let settingsConfig = null;

function escapeAttr(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/[\r\n]/g, "");
}

async function openSettings() {
  if (!invoke) return notAvailable();

  try {
    settingsConfig = await invoke("get_config");
  } catch (error) {
    console.error("Failed to load config:", error);
    toastError("Could not load settings.");
    return;
  }

  const modal = document.getElementById("settings-modal");
  if (!modal) return;
  const content = modal.querySelector(".modal-content");
  if (!content) return;

  const { replay, ai } = settingsConfig;
  const providers = ["local", "openai", "anthropic"];
  const fieldStyle =
    "width: 100%; margin: 4px 0 12px; padding: 6px 8px; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; color: var(--text);";

  content.innerHTML = `
    <h3>⚙️ Settings</h3>

    <h4 style="color: #8d7bff; margin-bottom: 4px;">Replay</h4>
    <label>Default speed (0.1–10)
      <input id="cfg-default-speed" type="number" step="0.1" min="0.1" max="10"
             value="${escapeAttr(replay.default_speed)}" style="${fieldStyle}">
    </label>
    <label>Max retry attempts
      <input id="cfg-max-retry" type="number" step="1" min="0"
             value="${escapeAttr(replay.max_retry_attempts)}" style="${fieldStyle}">
    </label>
    <label>Retry backoff (ms)
      <input id="cfg-backoff-ms" type="number" step="50" min="0"
             value="${escapeAttr(replay.retry_backoff_ms)}" style="${fieldStyle}">
    </label>
    <label>Retry backoff multiplier
      <input id="cfg-backoff-mult" type="number" step="0.1" min="1"
             value="${escapeAttr(replay.retry_backoff_multiplier)}" style="${fieldStyle}">
    </label>

    <h4 style="color: #8d7bff; margin: 12px 0 4px;">AI</h4>
    <label style="display: flex; align-items: center; gap: 8px; margin-bottom: 12px;">
      <input id="cfg-ai-enabled" type="checkbox" ${ai.enabled ? "checked" : ""}>
      AI features enabled
    </label>
    <label>Provider
      <select id="cfg-ai-provider" style="${fieldStyle}">
        ${providers
          .map(
            (p) =>
              `<option value="${p}" ${p === ai.provider ? "selected" : ""}>${p}</option>`,
          )
          .join("")}
      </select>
    </label>
    <label>Model
      <input id="cfg-ai-model" type="text" value="${escapeAttr(ai.model)}" style="${fieldStyle}">
    </label>
    <label>API endpoint (optional)
      <input id="cfg-ai-endpoint" type="text" placeholder="provider default"
             value="${escapeAttr(ai.api_endpoint ?? "")}" style="${fieldStyle}">
    </label>
    <p class="panel__hint" style="margin: 4px 0 12px;">API keys come from environment variables (OPENAI_API_KEY / ANTHROPIC_API_KEY), never stored here.</p>

    <div style="display: flex; gap: 8px; margin-top: 8px;">
      <button class="btn btn--primary btn--small" data-save-config>Save</button>
      <button class="btn btn--ghost btn--small" data-close-modal="settings-modal">Cancel</button>
    </div>
  `;

  showModal(modal);
}

async function saveSettings() {
  if (!invoke || !settingsConfig) return;

  const num = (id, fallback) => {
    const v = parseFloat(document.getElementById(id)?.value);
    return Number.isFinite(v) ? v : fallback;
  };

  // Merge edits into the cached full config so the backend receives a
  // complete, valid GhostConfig.
  settingsConfig.replay.default_speed = num("cfg-default-speed", settingsConfig.replay.default_speed);
  settingsConfig.replay.max_retry_attempts = Math.round(num("cfg-max-retry", settingsConfig.replay.max_retry_attempts));
  settingsConfig.replay.retry_backoff_ms = Math.round(num("cfg-backoff-ms", settingsConfig.replay.retry_backoff_ms));
  settingsConfig.replay.retry_backoff_multiplier = num("cfg-backoff-mult", settingsConfig.replay.retry_backoff_multiplier);

  settingsConfig.ai.enabled = !!document.getElementById("cfg-ai-enabled")?.checked;
  settingsConfig.ai.provider = document.getElementById("cfg-ai-provider")?.value || settingsConfig.ai.provider;
  settingsConfig.ai.model = document.getElementById("cfg-ai-model")?.value || settingsConfig.ai.model;
  const endpoint = document.getElementById("cfg-ai-endpoint")?.value?.trim();
  settingsConfig.ai.api_endpoint = endpoint ? endpoint : null;

  try {
    await invoke("update_config", { config: settingsConfig });
    // Reflect the new default speed in the picker and live state.
    playbackSpeed = settingsConfig.replay.default_speed;
    const speedSelect = document.getElementById("speedSelect");
    if (speedSelect) speedSelect.value = String(playbackSpeed);
    closeModal("settings-modal");
    showNotification("Settings saved.");
  } catch (error) {
    console.error("Failed to save config:", error);
    toastError(`Could not save settings: ${error}`);
  }
}

// On startup, reflect the persisted default speed in the picker.
async function syncSpeedFromConfig() {
  if (!invoke) return;
  try {
    const config = await invoke("get_config");
    const speed = config?.replay?.default_speed;
    if (typeof speed === "number") {
      playbackSpeed = speed;
      const speedSelect = document.getElementById("speedSelect");
      if (speedSelect) speedSelect.value = String(speed);
    }
  } catch (error) {
    console.error("Failed to sync speed from config:", error);
  }
}

// ===== Event timeline =====
// (Cloud sync UI removed: Ghost is local-only. The backend stubs remain but
// are not exposed — re-add a panel here only once a real, opt-in backend
// exists and the privacy messaging is updated to match.)

// InputEvent serializes as an externally-tagged enum: {"MouseClick": {x, y, …}}.
// Normalize to (type, data) before rendering — reading event.x directly is a bug.
function normalizeEvent(event) {
  if (event.type) return { type: event.type, data: event };
  const type = Object.keys(event)[0];
  return { type, data: event[type] ?? {} };
}

function describeEvent(event) {
  const { type, data } = normalizeEvent(event);

  switch (type) {
    case "MouseClick": {
      // button: 0=left down, 1=left up, 2=right down, 3=right up.
      // Only show downs — ups are replay detail, not user intent.
      if (data.button === 1 || data.button === 3) return null;
      const kind = data.button === 2 ? "Right-clicked" : "Clicked";
      const el = data.element;
      let description;
      if (el && (el.name || el.role)) {
        const role = (el.role_description || el.role || "element").replace(/^AX/, "");
        const name = el.name ? ` "${el.name}"` : "";
        const app = el.app && el.app !== "Unknown" ? ` in ${el.app}` : "";
        const win = el.window_title ? ` — window "${el.window_title}"` : "";
        description = `${kind} ${role}${name}${app}${win}`;
      } else {
        description = `${kind} at (${data.x}, ${data.y})`;
      }
      if (data.semantic_tag) {
        description += ` [AI: ${data.semantic_tag.action} on ${data.semantic_tag.target}]`;
      }
      return description;
    }
    case "Key": {
      if (data.action !== "Down") return null; // hide key-ups
      const mods = [];
      if (data.modifiers & 0x08) mods.push("⌘");
      if (data.modifiers & 0x02) mods.push("⌃");
      if (data.modifiers & 0x04) mods.push("⌥");
      if (data.modifiers & 0x01) mods.push("⇧");
      const prefix = mods.length ? mods.join("") + " + " : "";
      if (data.chars && data.chars.trim()) return `Typed ${prefix}"${data.chars}"`;
      return `Pressed ${prefix}key ${data.code}`;
    }
    case "Scroll":
      return `Scrolled (${data.dx}, ${data.dy})`;
    case "Delay":
      return `Waited ${data.ms}ms`;
    case "Wait":
      return `Wait: ${getConditionDescription(data.condition)}`;
    case "VisualCheck":
      return `Visual check (threshold ${data.threshold})`;
    case "Variable":
      return `Variable: ${data.name} = ${data.value_template}`;
    default:
      return JSON.stringify(event);
  }
}

function scheduleTimelineFlush() {
  if (timelineFlushScheduled) return;
  timelineFlushScheduled = true;
  requestAnimationFrame(() => {
    timelineFlushScheduled = false;
    const batch = pendingTimelineEvents.splice(0, 80);
    for (const item of batch) addEventToTimeline(item);
    trimTimeline();
    updateWorkflowHealth();
    if (pendingTimelineEvents.length > 0) scheduleTimelineFlush();
  });
}

function queueTimelineEvent(event) {
  pendingTimelineEvents.push(event);
  scheduleTimelineFlush();
}

function trimTimeline() {
  const timelineEl = document.getElementById("events-timeline");
  if (!timelineEl) return;
  while (timelineEl.children.length > MAX_TIMELINE_ITEMS) {
    timelineEl.removeChild(timelineEl.firstElementChild);
  }
}

function addEventToTimeline(event) {
  const timelineEl = document.getElementById("events-timeline");
  if (!timelineEl) return;

  const description = describeEvent(event);
  if (description === null) return; // filtered (mouse-up / key-up noise)

  const empty = timelineEl.querySelector(".events-timeline__empty");
  if (empty) empty.remove();

  const item = document.createElement("div");
  item.className = "timeline-item";
  item.textContent = description;
  timelineEl.appendChild(item);
  timelineEl.scrollTop = timelineEl.scrollHeight;
}

function getConditionDescription(condition) {
  if (!condition) return "Unknown condition";
  switch (condition.type) {
    case "ElementVisible":
      return `ElementVisible: ${condition.selector?.name || "element"}`;
    case "ElementExists":
      return `ElementExists: ${condition.selector?.name || "element"}`;
    case "TextPresent":
      return `TextPresent: "${condition.text || ""}"`;
    case "ImageMatches":
      return `ImageMatches: threshold=${condition.threshold || 0.9}`;
    case "Custom":
      return `Custom: ${condition.js_expression || ""}`;
    default:
      return JSON.stringify(condition);
  }
}

function updateRecordingUI() {
  const statusEl = document.getElementById("recording-status");
  const recordBtn = document.getElementById("recordBtn");
  const stopBtn = document.getElementById("stopBtn");
  const replayBtn = document.getElementById("replayBtn");
  const replayReliableBtn = document.getElementById("replayReliableBtn");
  const cancelBtn = document.getElementById("cancelBtn");
  const pauseBtn = document.getElementById("pauseBtn");
  const resumeBtn = document.getElementById("resumeBtn");

  if (statusEl) {
    // Drive the live glow classes (defined in app.css) off the same state.
    statusEl.classList.toggle("recording-status--live", isRecording);
    statusEl.classList.toggle("recording-status--playing", isPlaying && !isPaused);
    if (isRecording) {
      statusEl.innerHTML = '<span class="pulse" aria-hidden="true"></span> Recording workflow...';
      statusEl.style.color = "#ef4444";
    } else if (isPlaying) {
      if (isPaused) {
        statusEl.innerHTML = '<span class="pulse" aria-hidden="true" style="animation:none"></span> Paused';
        statusEl.style.color = "#f59e0b";
      } else {
        statusEl.innerHTML = '<span class="pulse" aria-hidden="true"></span> Playing...';
        statusEl.style.color = "#8d7bff";
      }
    } else {
      statusEl.innerHTML = '<span class="pulse" aria-hidden="true" style="display:none"></span> Ready to record';
      statusEl.style.color = "#22c55e";
    }
  }

  if (recordBtn) recordBtn.disabled = isRecording || isPlaying;
  if (stopBtn) stopBtn.disabled = !isRecording;
  if (replayBtn) replayBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  if (replayReliableBtn) replayReliableBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  const dryRunBtn = document.getElementById("dryRunBtn");
  if (dryRunBtn) dryRunBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  const stepReplayBtn = document.getElementById("stepReplayBtn");
  if (stepReplayBtn) stepReplayBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  const retryFailedBtn = document.getElementById("retryFailedBtn");
  if (retryFailedBtn) {
    retryFailedBtn.hidden = lastFailedStep === null;
    retryFailedBtn.disabled = isRecording || isPlaying;
  }
  const guardAuditBtn = document.getElementById("guardAuditBtn");
  if (guardAuditBtn) guardAuditBtn.disabled = isRecording || recordedEvents.length === 0;
  if (cancelBtn) cancelBtn.disabled = !isPlaying;
  if (pauseBtn) pauseBtn.disabled = !isPlaying || isPaused;
  if (resumeBtn) resumeBtn.disabled = !isPlaying || !isPaused;
  updateWorkflowHealth();
  updateMissionProgress();
}

function updateWorkflowHealth() {
  const steps = recordedEvents.map(describeEvent).filter(Boolean);
  const healthSteps = document.getElementById("healthSteps");
  const healthDuration = document.getElementById("healthDuration");
  const healthSignals = document.getElementById("healthSignals");
  if (healthSteps) healthSteps.textContent = String(steps.length);

  const durationMs = recordedEvents.reduce((sum, ev) => {
    const { type, data } = normalizeEvent(ev);
    return type === "Delay" ? sum + Number(data.ms || 0) : sum;
  }, 0);
  if (healthDuration) healthDuration.textContent = durationMs >= 1000 ? `${(durationMs / 1000).toFixed(1)}s` : `${durationMs}ms`;

  const richEvents = recordedEvents.filter((ev) => normalizeEvent(ev).data?.element?.name || normalizeEvent(ev).data?.element?.role).length;
  if (healthSignals) {
    if (recordedEvents.length === 0) healthSignals.textContent = "Empty";
    else if (richEvents >= Math.max(1, recordedEvents.length * 0.25)) healthSignals.textContent = "Strong";
    else healthSignals.textContent = "Basic";
  }
}

function updateMissionProgress({ permissionsGranted } = {}) {
  const mark = (step, done) => {
    const el = document.querySelector(`[data-mission-step="${step}"]`);
    if (el) el.classList.toggle("is-complete", !!done);
  };
  const permissionDone = typeof permissionsGranted === "boolean" ? permissionsGranted : document.getElementById("perm-banner")?.hidden;
  mark("permissions", permissionDone);
  mark("record", recordedEvents.length > 0);
  mark("audit", guardAuditCompleted);
  mark("replay", hasReplayedCurrentWorkflow || hasSavedCurrentWorkflow);
}

// ===== Smart Observer mode =====

let observerUpdateInterval = null;

async function startSmartObserver() {
  if (!invoke) return notAvailable();

  try {
    await invoke("start_observer");
    showInsight("Pattern observer started. Ghost will look for repeatable actions locally.");
    startObserverUIUpdate();
  } catch (error) {
    console.error("Failed to start observer:", error);
    toastError("Failed to start observer: " + error);
  }
}

async function stopSmartObserver() {
  if (!invoke) return;

  try {
    await invoke("stop_observer");
    showInsight("Pattern observer stopped.");
    if (observerUpdateInterval) {
      clearInterval(observerUpdateInterval);
      observerUpdateInterval = null;
    }
  } catch (error) {
    console.error("Failed to stop observer:", error);
  }
}

async function checkObserverStatus() {
  if (!invoke) return false;

  try {
    return await invoke("is_observer_active");
  } catch (error) {
    console.error("Failed to check observer status:", error);
    return false;
  }
}

function startObserverUIUpdate() {
  if (observerUpdateInterval) clearInterval(observerUpdateInterval);

  observerUpdateInterval = setInterval(async () => {
    const active = await checkObserverStatus();
    if (!active) {
      clearInterval(observerUpdateInterval);
      observerUpdateInterval = null;
    }
  }, 2000);
}

async function observeCurrentSession() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded to observe");
    return;
  }

  try {
    const appName = (await ghostPrompt("Which app were you using?", "Unknown App")) || "Unknown";
    const patternsFound = await invoke("observe_events", { events: recordedEvents, appName: appName });
    showNotification(`Found ${patternsFound} learned pattern(s) from ${appName}.`);

    const suggestions = await invoke("get_proactive_suggestions");
    if (suggestions.length > 0) displaySuggestions(suggestions);
  } catch (error) {
    console.error("Failed to observe events:", error);
    toastError("Failed to observe: " + error);
  }
}

async function generateGeekInsights() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    const appName = (await ghostPrompt("Which app are you analyzing?", "Unknown App")) || "Unknown";
    const insights = await invoke("generate_geek_insights", { events: recordedEvents, appName: appName });
    displayGeekInsights(insights, appName);
  } catch (error) {
    console.error("Failed to generate geek insights:", error);
    toastError("Failed to generate insights: " + error);
  }
}

function displaySuggestions(suggestions) {
  const modal = document.getElementById("analysis-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>🤖 Proactive Automation Suggestions</h3>
    ${suggestions.map((s, i) => `
      <div style="margin: 12px 0; padding: 12px; background: rgba(139, 123, 255, 0.1); border-radius: 8px; border-left: 3px solid #8d7bff;">
        <p><strong>${i + 1}. ${escapeHtml(s.suggestion)}</strong></p>
        <p style="font-size: 0.9rem; color: #9ca3af;">Suggested workflow: <code>${escapeHtml(s.suggested_workflow_name)}</code></p>
        <p style="font-size: 0.85rem;">Confidence: ${(s.confidence * 100).toFixed(1)}%</p>
        <button data-create-workflow-from-suggestion data-workflow-name="${escapeAttr(s.suggested_workflow_name)}" data-pattern-id="${escapeAttr(s.pattern_id)}" style="margin-top: 8px; font-size: 0.85rem;">Create This Workflow</button>
      </div>
    `).join("")}
    <button data-close-modal="analysis-modal">Close</button>
  `;

  showModal(modal);
}

async function createWorkflowFromSuggestion(name) {
  if (recordedEvents.length === 0) return;

  try {
    await invoke("save_workflow", { name, events: recordedEvents });
    closeModal("analysis-modal");
    showNotification(`Workflow "${name}" created.`);
  } catch (error) {
    console.error("Failed to save workflow:", error);
  }
}

function displayGeekInsights(insights, appName) {
  const modal = document.getElementById("analysis-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>🔧 Geek Mode: Technical Insights for ${escapeHtml(appName)}</h3>
    <div style="margin: 12px 0;">
      <h4 style="color: #8d7bff;">Performance Metrics</h4>
      <p>Total Duration: ${insights.performance_metrics.total_duration_ms}ms</p>
      <p>Avg Delay: ${insights.performance_metrics.avg_delay_ms.toFixed(2)}ms</p>
      ${insights.performance_metrics.bottleneck_events.length > 0 ? `
        <p>Bottleneck Events: ${escapeHtml(insights.performance_metrics.bottleneck_events.join(", "))}</p>
      ` : ""}
    </div>
    <div style="margin: 12px 0;">
      <h4 style="color: #8d7bff;">Event Timing Analysis</h4>
      <table style="width: 100%; font-size: 0.85rem;">
        <tr style="border-bottom: 1px solid #374151;">
          <th>Index</th><th>Action</th><th>Delay Before</th>
        </tr>
        ${insights.event_timing_analysis.slice(0, 10).map((t) => `
          <tr style="border-bottom: 1px solid #374151;">
            <td>${t.event_index}</td>
            <td>${escapeHtml(t.estimated_action)}</td>
            <td>${t.delay_before_ms}ms</td>
          </tr>
        `).join("")}
        ${insights.event_timing_analysis.length > 10 ? `<tr><td colspan="3">... and ${insights.event_timing_analysis.length - 10} more</td></tr>` : ""}
      </table>
    </div>
    <button data-close-modal="analysis-modal">Close</button>
  `;

  showModal(modal);
}

// ===== Visual regression =====

async function replayWithVisualCheck() {
  if (!invoke) return notAvailable();
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    const appName = await ghostPrompt("Baseline name", "default_app");
    if (appName) await invoke("capture_baseline_screenshot", { name: appName });

    const visualChecks = [
      { event_index: recordedEvents.length - 1, name: "Final State", baseline_screenshot_path: appName ? `${appName}.png` : null, threshold: 0.95 },
    ];

    const success = await invoke("replay_with_visual_check", { events: recordedEvents, visualChecks: visualChecks });
    showNotification(success ? "Replay completed with visual check." : "Replay was cancelled.");
  } catch (error) {
    console.error("Failed to replay with visual check:", error);
    toastError("Replay failed: " + error);
  }
}

async function captureBaseline() {
  if (!invoke) return;

  const name = await ghostPrompt("Baseline name");
  if (!name) return;

  try {
    await invoke("capture_baseline_screenshot", { name });
    showNotification(`Baseline "${name}" captured.`);
  } catch (error) {
    console.error("Failed to capture baseline:", error);
    toastError("Capture failed: " + error);
  }
}

// ===== Data sources =====

function loadDemoWorkflow() {
  const now = Date.now();
  resetReplayInspectionState();
  recordedEvents = [
    { MouseClick: { x: 420, y: 280, button: 0, timestamp: now, element: { app: "Demo CRM", role: "AXButton", role_description: "button", name: "Export CSV" } } },
    { Delay: { ms: 450 } },
    { Key: { code: 36, chars: "weekly_report", action: "Down", modifiers: 0, timestamp: now + 450 } },
    { Delay: { ms: 300 } },
    { MouseClick: { x: 640, y: 520, button: 0, timestamp: now + 750, element: { app: "Demo Mail", role: "AXButton", role_description: "button", name: "Send" } } },
  ];
  guardAuditCompleted = false;
  latestGuardReport = null;
  hasReplayedCurrentWorkflow = false;
  hasSavedCurrentWorkflow = false;
  refreshTimeline();
  updateRecordingUI();
  showInsight("Demo loaded. This is the shape of a useful Ghost workflow: clear steps, timing, and UI context.");
  showNotification("Demo workflow loaded — audit it, then try recording your own.");
}

async function createDataSource() {
  if (!invoke) return;

  const name = await ghostPrompt("Data source name");
  if (!name) return;

  const type = (await ghostPick("Data source type", ["environment", "csv", "json"])) || "environment";
  let path = null;
  if (type === "csv" || type === "json") path = await ghostPrompt("Path to data file");

  try {
    await invoke("create_data_source", { name, sourceType: type, path });
    showNotification(`Data source "${name}" created.`);
  } catch (error) {
    console.error("Failed to create data source:", error);
    toastError("Create failed: " + error);
  }
}

async function loadVariablesFromSource() {
  if (!invoke) return;

  const name = await ghostPrompt("Data source name");
  if (!name) return;

  try {
    const variables = await invoke("load_variables", { dataSourceName: name });
    showNotification(`Loaded ${Object.keys(variables).length} variables.`);
    console.log("Variables:", variables);
  } catch (error) {
    console.error("Failed to load variables:", error);
    toastError("Load failed: " + error);
  }
}

// ===== Wire up the UI =====

// ===== Ghost Organizer =====
// The wedge product's trust pipeline, surfaced end-to-end:
//   Scan -> Preview -> Approve -> Organize -> Audit -> Undo
// `organizer_plan` is read-only (mutates nothing); `organizer_execute` and
// `organizer_undo` mutate only inside an approved Zone and the backend
// re-checks policy on every action, so the UI never decides what is safe.

let organizerZones = [];
let organizerSelectedZoneId = null;
let organizerHasReviewedPlan = false;

async function organizerInit() {
  if (!invoke) return; // static mode: the panel stays inert
  await organizerRefreshZones();
}

function organizerSelectedZone() {
  return organizerZones.find((z) => z.id === organizerSelectedZoneId) || null;
}

async function organizerRefreshZones() {
  try {
    organizerZones = await invoke("organizer_list_zones");
  } catch (err) {
    organizerZones = [];
    toastError("Could not load Zones: " + err);
  }
  const select = document.getElementById("organizerZoneSelect");
  if (!select) return;

  if (organizerZones.length === 0) {
    select.innerHTML = `<option value="">No Zones yet — create one</option>`;
    organizerSelectedZoneId = null;
  } else {
    if (!organizerZones.some((z) => z.id === organizerSelectedZoneId)) {
      organizerSelectedZoneId = organizerZones[0].id;
    }
    select.innerHTML = organizerZones
      .map(
        (z) =>
          `<option value="${escapeAttr(z.id)}"${z.id === organizerSelectedZoneId ? " selected" : ""}>${escapeHtml(z.name)}</option>`,
      )
      .join("");
  }
  // Switching Zones invalidates any previewed plan.
  organizerHasReviewedPlan = false;
  await organizerRefreshRules();
}

async function organizerRefreshRules() {
  const list = document.getElementById("organizerRulesList");
  if (!list) return;
  const zone = organizerSelectedZone();
  if (!zone) {
    list.textContent = "Create a Zone, then add the folder you want to organize.";
    organizerUpdateButtons([]);
    return;
  }
  let rules = [];
  try {
    rules = await invoke("organizer_list_folder_rules", { zoneId: zone.id });
  } catch (err) {
    toastError("Could not load folders: " + err);
  }
  if (rules.length === 0) {
    list.textContent = "No folders in this Zone yet — add one above.";
  } else {
    list.innerHTML = rules
      .map((r) => {
        const grants = [
          r.can_read && "read",
          r.can_create && "create",
          r.can_move && "move",
          r.can_rename && "rename",
        ]
          .filter(Boolean)
          .join(", ");
        return `<span class="organizer-rule-chip">${escapeHtml(r.path)} <em>(${escapeHtml(grants || "no permissions")})</em></span>`;
      })
      .join("");
  }
  organizerUpdateButtons(rules);
}

function organizerUpdateButtons(rules) {
  const scanBtn = document.getElementById("organizerScanBtn");
  const runBtn = document.getElementById("organizerRunBtn");
  const hasFolders = Array.isArray(rules) && rules.length > 0;
  if (scanBtn) scanBtn.disabled = !organizerSelectedZone() || !hasFolders;
  if (runBtn) runBtn.disabled = !organizerHasReviewedPlan;
}

async function organizerCreateZone() {
  if (!invoke) return notAvailable();
  const name = await ghostPrompt("Name this Zone (e.g. Downloads cleanup)", "", "Zone name");
  if (!name) return;
  try {
    const zone = await invoke("organizer_create_zone", { name, description: null });
    organizerSelectedZoneId = zone.id;
    await organizerRefreshZones();
    showNotification(`Zone "${zone.name}" created. Add the folder to organize.`, "info");
  } catch (err) {
    toastError("Could not create Zone: " + err);
  }
}

async function organizerAddFolder() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Create a Zone first.");
  const path = document.getElementById("organizerFolderPath")?.value?.trim();
  if (!path) return toastError("Enter a folder path to add.");

  const rule = {
    path,
    can_read: !!document.getElementById("permRead")?.checked,
    can_create: !!document.getElementById("permCreate")?.checked,
    can_rename: !!document.getElementById("permRename")?.checked,
    can_move: !!document.getElementById("permMove")?.checked,
    can_copy: false,
    can_delete: false, // Ghost never deletes through the Organizer.
  };
  try {
    await invoke("organizer_add_folder_rule", { zoneId: zone.id, rule });
    const input = document.getElementById("organizerFolderPath");
    if (input) input.value = "";
    organizerHasReviewedPlan = false;
    await organizerRefreshRules();
    showNotification("Folder added to the Zone.", "info");
  } catch (err) {
    toastError("Could not add folder: " + err);
  }
}

// Render a capability as a compact, human-readable action line.
function organizerDescribeCapability(cap) {
  const base = (p) => String(p ?? "").split(/[\\/]/).pop() || p;
  switch (cap.kind) {
    case "create_folder":
      return `Create folder <code>${escapeHtml(cap.path)}</code>`;
    case "move_file":
      return `Move <code>${escapeHtml(base(cap.from))}</code> → <code>${escapeHtml(cap.to)}</code>`;
    case "rename_file":
      return `Rename <code>${escapeHtml(base(cap.from))}</code> → <code>${escapeHtml(base(cap.to))}</code>`;
    default:
      return `<code>${escapeHtml(cap.kind || "action")}</code>`;
  }
}

function organizerDecisionBadge(decision) {
  if (!decision) return "";
  if (decision.decision === "allow")
    return `<span class="org-badge org-badge--allow">Allowed</span>`;
  if (decision.decision === "deny")
    return `<span class="org-badge org-badge--deny" title="${escapeAttr(decision.reason || "")}">Denied</span>`;
  if (decision.decision === "require_confirmation")
    return `<span class="org-badge org-badge--confirm">Needs approval · ${escapeHtml(decision.risk || "")}</span>`;
  return "";
}

async function organizerScan() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Create a Zone first.");
  const result = document.getElementById("organizerResult");
  if (result) result.innerHTML = `<p class="organizer-muted">Scanning… nothing has been changed.</p>`;

  let plan;
  try {
    plan = await invoke("organizer_plan", { zoneId: zone.id });
  } catch (err) {
    if (result) result.innerHTML = "";
    return toastError("Scan failed: " + err);
  }
  organizerRenderPlan(plan);
  organizerHasReviewedPlan = plan.actions.length > 0;
  organizerUpdateButtons(await safeRules(zone.id));
  showInsight("Preview ready. Nothing moved yet — review each step, then approve.");
}

async function safeRules(zoneId) {
  try {
    return await invoke("organizer_list_folder_rules", { zoneId });
  } catch {
    return [];
  }
}

function organizerRenderPlan(plan) {
  const result = document.getElementById("organizerResult");
  if (!result) return;
  const s = plan.summary || {};
  const rows = plan.actions
    .map((a) => {
      const conflict = a.conflict
        ? `<span class="org-flag" title="${escapeAttr(a.conflict.original_target || "")}">conflict</span>`
        : "";
      const low =
        typeof a.confidence === "number" && a.confidence <= 0.5
          ? `<span class="org-flag">low confidence</span>`
          : "";
      return `<tr>
        <td>${organizerDescribeCapability(a.capability)} ${conflict} ${low}</td>
        <td>${organizerDecisionBadge(a.decision)}</td>
      </tr>`;
    })
    .join("");

  const skipped = (plan.skipped || []).length
    ? `<p class="organizer-muted">${plan.skipped.length} file(s) left alone (already organized or no destination).</p>`
    : "";

  result.innerHTML = `
    <div class="organizer-summary">
      <strong>${s.files_scanned ?? 0}</strong> scanned ·
      <strong>${s.move_file ?? 0}</strong> moves ·
      <strong>${s.create_folder ?? 0}</strong> new folders ·
      <strong>${s.conflicts ?? 0}</strong> conflicts ·
      <strong>${s.denied ?? 0}</strong> denied
    </div>
    ${
      plan.actions.length
        ? `<table class="organizer-table"><thead><tr><th>Proposed action</th><th>Policy</th></tr></thead><tbody>${rows}</tbody></table>`
        : `<p class="organizer-muted">No actions proposed for this Zone.</p>`
    }
    ${skipped}
    <p class="organizer-note">Preview only — approve below to apply. Ghost writes an undo journal before moving any file.</p>`;
}

async function organizerRun() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return;
  const ok = await ghostConfirm(
    `Organize "${zone.name}" now? Files move into category folders. You can undo this afterward.`,
  );
  if (!ok) return;

  let res;
  try {
    res = await invoke("organizer_execute", { zoneId: zone.id });
  } catch (err) {
    return toastError("Organize failed: " + err);
  }
  const r = res.report || {};
  const auditRows = (r.audit || [])
    .map((e) => {
      const outcome = e.outcome?.outcome || "?";
      const detail = e.outcome?.reason || e.outcome?.error || "";
      return `<li><span class="org-outcome org-outcome--${escapeAttr(outcome)}">${escapeHtml(outcome)}</span> ${organizerDescribeCapability(e.capability || {})}${detail ? ` — <em>${escapeHtml(detail)}</em>` : ""}</li>`;
    })
    .join("");

  const result = document.getElementById("organizerResult");
  if (result) {
    result.innerHTML = `
      <div class="organizer-summary organizer-summary--done">
        ✓ <strong>${r.applied ?? 0}</strong> applied ·
        <strong>${r.skipped ?? 0}</strong> skipped ·
        <strong>${r.failed ?? 0}</strong> failed
      </div>
      <button class="btn btn--ghost btn--small" id="organizerUndoLastBtn" type="button" data-exec-id="${escapeAttr(res.execution_id)}">Undo this run</button>
      <h3 class="organizer-subhead">Audit log</h3>
      <ul class="organizer-audit">${auditRows || "<li>No actions recorded.</li>"}</ul>`;
    const undoBtn = document.getElementById("organizerUndoLastBtn");
    if (undoBtn) undoBtn.addEventListener("click", () => organizerUndo(res.execution_id));
  }
  organizerHasReviewedPlan = false;
  organizerUpdateButtons(await safeRules(zone.id));
  showNotification(`Organized: ${r.applied ?? 0} change(s) applied.`, "info");
}

async function organizerUndo(executionId) {
  if (!invoke) return notAvailable();
  if (!executionId) return;
  try {
    const report = await invoke("organizer_undo", { executionId });
    showNotification(
      `Undo complete: ${report.reverted} reverted, ${report.skipped} skipped, ${report.failed} failed.`,
      report.failed ? "error" : "info",
    );
    await organizerRefreshRules();
  } catch (err) {
    toastError("Undo failed: " + err);
  }
}

async function organizerShowHistory() {
  if (!invoke) return notAvailable();
  let history = [];
  try {
    history = await invoke("organizer_list_executions");
  } catch (err) {
    return toastError("Could not load history: " + err);
  }
  const modal = document.getElementById("analysis-modal");
  const content = modal?.querySelector(".modal-content");
  if (!content) return;
  const zoneName = (id) => organizerZones.find((z) => z.id === id)?.name || id;
  const rows = history.length
    ? history
        .map(
          (h) => `<li>
            <div><strong>${escapeHtml(zoneName(h.zone_id))}</strong> — ${h.applied} applied, ${h.skipped} skipped, ${h.failed} failed</div>
            <button class="btn btn--ghost btn--small" data-undo-exec="${escapeAttr(h.id)}">Undo</button>
          </li>`,
        )
        .join("")
    : "<li>No runs yet.</li>";
  content.innerHTML = `
    <h3 style="margin-top:0">Organizer history</h3>
    <ul class="organizer-history">${rows}</ul>
    <div style="margin-top:16px"><button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button></div>`;
  content.querySelectorAll("[data-undo-exec]").forEach((btn) =>
    btn.addEventListener("click", async () => {
      await organizerUndo(btn.dataset.undoExec);
      closeModal("analysis-modal");
    }),
  );
  showModal(modal);
}

// Replay history: surface past replay runs (status, duration, failure reason)
// recorded by the engine's execution history. Read-only; mirrors the Organizer
// history modal.
async function showReplayHistory() {
  if (!invoke) return notAvailable();
  let history = [];
  try {
    history = await invoke("get_replay_history", { limit: 50 });
  } catch (err) {
    return toastError("Could not load replay history: " + err);
  }
  const modal = document.getElementById("analysis-modal");
  const content = modal?.querySelector(".modal-content");
  if (!content) return;

  const fmtWhen = (secs) =>
    secs ? new Date(secs * 1000).toLocaleString() : "—";
  const fmtDur = (ms) =>
    ms || ms === 0 ? `${(ms / 1000).toFixed(1)}s` : "—";

  // Summarize the run's resolution trace: how each click found its target,
  // with coordinate-fallback steps called out individually — this is where
  // "why did this run behave that way" gets answered.
  const traceHtml = (h) => {
    const trace = h.step_trace || [];
    if (!trace.length) return "";
    const count = (kind) => trace.filter((t) => t.kind === kind).length;
    const parts = [];
    if (count("RecordedPoint")) parts.push(`${count("RecordedPoint")} found in place`);
    if (count("SpiralReresolved")) parts.push(`${count("SpiralReresolved")} re-resolved nearby`);
    if (count("CoordinateFallback")) parts.push(`${count("CoordinateFallback")} lost their element`);
    if (count("NoDescriptor")) parts.push(`${count("NoDescriptor")} coordinate-only`);
    const risky = trace.filter(
      (t) => t.kind === "CoordinateFallback" || t.kind === "NoDescriptor"
    );
    const details = risky
      .map((t) => {
        const label = t.target_name
          ? `"${escapeHtml(t.target_name)}" not found — clicked recorded point`
          : "no element recorded — clicked fixed point";
        return `<div class="replay-meta replay-trace__fallback">Step ${t.step_index + 1}: ${label} (${t.point[0]}, ${t.point[1]})</div>`;
      })
      .join("");
    return `<div class="replay-meta">Targets: ${escapeHtml(parts.join(" · "))}</div>${details}`;
  };

  const rows = history.length
    ? history
        .map((h) => {
          const status = String(h.status || "");
          const cls = status.toLowerCase();
          const err = h.error_message
            ? ` — ${escapeHtml(h.error_message)}`
            : "";
          return `<li>
            <div><strong>${escapeHtml(h.workflow_name)}</strong>
              <span class="replay-status replay-status--${escapeAttr(cls)}">${escapeHtml(status)}</span></div>
            <div class="replay-meta">${escapeHtml(fmtWhen(h.start_time))} · ${h.events_processed} events · ${escapeHtml(fmtDur(h.duration_ms))}${err}</div>
            ${traceHtml(h)}
          </li>`;
        })
        .join("")
    : "<li>No replays yet.</li>";

  content.innerHTML = `
    <h3 style="margin-top:0">Replay history</h3>
    <ul class="replay-history">${rows}</ul>
    <div style="margin-top:16px"><button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button></div>`;
  showModal(modal);
}

// ===== Replay inspection =====
// Week 2 of the trust roadmap: make replay inspectable. Dry-run preview shows
// "what Ghost will do next" before anything executes; live per-step status is
// polled from get_replay_progress while a replay runs; step-by-step replay and
// retry-from-failed-step reuse the stable replay_workflow command over event
// slices (press/release pairs stay together, preserving click semantics).

let lastFailedStep = null; // event index the last replay failed on, or null
let stepReplayCursor = 0; // next event index for step-by-step replay
let replayProgressTimer = null;

// After a replay, explain how click targets were actually resolved (from the
// run's persisted resolution trace) so fallback decisions are never silent.
async function summarizeLastReplayResolution() {
  if (!invoke) return;
  try {
    const [latest] = await invoke("get_replay_history", { limit: 1 });
    const trace = latest?.step_trace || [];
    if (!trace.length) return;
    const lost = trace.filter((t) => t.kind === "CoordinateFallback").length;
    const blind = trace.filter((t) => t.kind === "NoDescriptor").length;
    const moved = trace.filter((t) => t.kind === "SpiralReresolved").length;
    if (lost + blind > 0) {
      showInsight(
        `${lost + blind} click(s) ran on raw coordinates` +
          (lost ? ` (${lost} couldn't find their element)` : "") +
          ". These steps are the most likely to break — open Replay History for the per-step trace."
      );
    } else if (moved > 0) {
      showInsight(`Every click found its element — ${moved} had moved and were re-resolved nearby.`);
    }
  } catch (error) {
    console.error("Could not summarize resolution trace:", error);
  }
}

// Called wherever recordedEvents is replaced wholesale — stale step cursors
// and failed-step markers must not point into a different workflow.
function resetReplayInspectionState() {
  lastFailedStep = null;
  stepReplayCursor = 0;
  const el = replayProgressEl();
  if (el) {
    el.hidden = true;
    el.textContent = "";
  }
}

function replayProgressEl() {
  return document.getElementById("replay-progress");
}

function startReplayProgressPolling(offset = 0) {
  const el = replayProgressEl();
  if (!el || !invoke) return;
  el.hidden = false;
  el.textContent = "Starting replay…";
  stopReplayProgressPolling(true);
  replayProgressTimer = setInterval(async () => {
    try {
      const p = await invoke("get_replay_progress");
      if (!p || !p.running) return;
      el.textContent = `Replaying step ${offset + p.current_step + 1} of ${offset + p.total_steps}…`;
    } catch {
      // Progress is cosmetic; never let a poll failure disturb the replay.
    }
  }, 200);
}

function stopReplayProgressPolling(silent = false) {
  if (replayProgressTimer) {
    clearInterval(replayProgressTimer);
    replayProgressTimer = null;
  }
  if (silent) return;
  const el = replayProgressEl();
  if (!el) return;
  if (lastFailedStep !== null) {
    el.textContent = `Replay failed at step ${lastFailedStep + 1}. You can retry from that step.`;
  } else {
    el.hidden = true;
    el.textContent = "";
  }
}

// After a failed replay, ask the engine which event it stopped on so the UI
// can mark it and offer "retry from failed step". `offset` maps slice-relative
// indices back to positions in recordedEvents.
async function captureFailedStep(offset) {
  if (!invoke) return;
  try {
    const p = await invoke("get_replay_progress");
    if (p && p.failed_step !== null && p.failed_step !== undefined) {
      lastFailedStep = offset + p.failed_step;
    }
  } catch (error) {
    console.error("Could not read failed step:", error);
  }
}

// Dry-run preview: render the backend's per-step "what will happen" report.
// Read-only — nothing executes until the user replays for real.
async function showDryRunPreview() {
  if (!invoke) return notAvailable();
  if (recordedEvents.length === 0) return toastError("No events recorded yet");

  let steps = [];
  try {
    steps = await invoke("dry_run_workflow", { events: recordedEvents });
  } catch (err) {
    return toastError("Could not build preview: " + err);
  }

  const modal = document.getElementById("analysis-modal");
  const content = modal?.querySelector(".modal-content");
  if (!content) return;

  const fallbackCount = steps.filter((s) => s.coordinate_fallback).length;
  const summary = fallbackCount
    ? `<p class="dry-run__warning">⚠️ ${fallbackCount} step(s) will click raw coordinates with no element to re-resolve — they break if the UI moves.</p>`
    : `<p class="dry-run__ok">Every click has an element target or replays a recorded release point.</p>`;

  const rows = steps
    .map((s) => {
      const target = s.target ? ` — ${escapeHtml(s.target)}` : "";
      const badge = s.coordinate_fallback
        ? ' <span class="dry-run__badge">coordinates only</span>'
        : "";
      return `<li>
        <div><strong>${s.index + 1}. ${escapeHtml(s.action)}</strong>${target}${badge}</div>
        <div class="replay-meta">${escapeHtml(s.detail)}</div>
      </li>`;
    })
    .join("");

  content.innerHTML = `
    <h3 style="margin-top:0">Replay preview — what Ghost will do</h3>
    ${summary}
    <ul class="replay-history dry-run">${rows}</ul>
    <div style="margin-top:16px"><button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button></div>`;
  showModal(modal);
}

// A "step" for step-by-step replay starts at a user-intent event; releases
// (mouse-up, key-up) ride along with the press that opened them so click and
// keystroke pairs never get split across steps.
function isPrimaryEvent(event) {
  const { type, data } = normalizeEvent(event);
  if (type === "MouseClick") return data.button === 0 || data.button === 2;
  if (type === "Key") return data.action === "Down";
  return true;
}

function nextStepBoundary(events, start) {
  let i = start + 1;
  while (i < events.length && !isPrimaryEvent(events[i])) i++;
  return i;
}

async function replayNextStep() {
  if (!invoke) return notAvailable();
  if (recordedEvents.length === 0) return toastError("No events recorded yet");
  if (isPlaying || isRecording) return;

  if (stepReplayCursor >= recordedEvents.length) stepReplayCursor = 0;
  // Approve the workflow once at the start of a stepped run, not per step.
  if (stepReplayCursor === 0 && !(await confirmGuardBeforeReplay())) return;

  const start = stepReplayCursor;
  const end = nextStepBoundary(recordedEvents, start);
  const slice = recordedEvents.slice(start, end);

  try {
    isPlaying = true;
    updateRecordingUI();
    await invoke("replay_workflow", { events: slice });
    stepReplayCursor = end;
    const remaining = recordedEvents.length - end;
    showInsight(
      remaining > 0
        ? `Replayed events ${start + 1}–${end} of ${recordedEvents.length}. ${remaining} event(s) left — step again to continue.`
        : `Replayed events ${start + 1}–${end}. End of workflow — the next step starts over.`
    );
  } catch (error) {
    console.error("Step replay failed:", error);
    toastError("Step replay failed: " + error);
    await captureFailedStep(start);
  } finally {
    isPlaying = false;
    updateRecordingUI();
  }
}

async function retryFromFailedStep() {
  if (!invoke) return notAvailable();
  if (lastFailedStep === null) return;
  if (lastFailedStep >= recordedEvents.length) {
    lastFailedStep = null;
    updateRecordingUI();
    return;
  }
  if (!(await confirmGuardBeforeReplay())) return;

  const offset = lastFailedStep;
  const slice = recordedEvents.slice(offset);
  try {
    isPlaying = true;
    lastFailedStep = null;
    updateRecordingUI();
    startReplayProgressPolling(offset);
    await invoke("replay_workflow", { events: slice });
    hasReplayedCurrentWorkflow = true;
    showInsight(`Retried from step ${offset + 1} and finished the workflow.`);
    await summarizeLastReplayResolution();
  } catch (error) {
    console.error("Retry from failed step failed:", error);
    toastError("Retry failed: " + error);
    await captureFailedStep(offset);
  } finally {
    isPlaying = false;
    stopReplayProgressPolling();
    updateRecordingUI();
  }
}

function wireUpControls() {
  const bind = (id, handler) => {
    const el = document.getElementById(id);
    if (el) el.addEventListener("click", handler);
  };

  bind("recordBtn", startRecording);
  bind("stopBtn", stopRecording);
  bind("replayBtn", replayWorkflow);
  bind("replayReliableBtn", replayWithReliability);
  bind("cancelBtn", cancelReplay);
  bind("pauseBtn", pauseReplay);
  bind("resumeBtn", resumeReplay);
  bind("inspectElementBtn", inspectElementAtCursor);
  bind("guardAuditBtn", () => runGhostGuardAudit());
  bind("demoWorkflowBtn", loadDemoWorkflow);
  bind("replayHistoryBtn", showReplayHistory);
  bind("dryRunBtn", showDryRunPreview);
  bind("stepReplayBtn", replayNextStep);
  bind("retryFailedBtn", retryFromFailedStep);

  bind("saveBtn", saveWorkflow);
  bind("saveAiBtn", saveWorkflowWithMetadata);
  bind("loadBtn", loadWorkflow);
  bind("analyzeBtn", analyzeWorkflow);
  bind("optimizeBtn", optimizeWorkflow);
  bind("generateAiBtn", generateWorkflowFromDescription);

  bind("startObserverBtn", startSmartObserver);
  bind("stopObserverBtn", stopSmartObserver);
  bind("observeSessionBtn", observeCurrentSession);
  bind("geekModeBtn", generateGeekInsights);

  bind("visualCheckBtn", replayWithVisualCheck);
  bind("captureBaselineBtn", captureBaseline);
  bind("newDataSourceBtn", createDataSource);
  bind("loadVariablesBtn", loadVariablesFromSource);

  bind("perm-grant", requestAccessibility);
  bind("settingsBtn", openSettings);
  bind("lockBtn", lockApp);

  // Ghost Organizer: the wedge product's trust pipeline.
  bind("organizerNewZoneBtn", organizerCreateZone);
  bind("organizerAddFolderBtn", organizerAddFolder);
  bind("organizerScanBtn", organizerScan);
  bind("organizerRunBtn", organizerRun);
  bind("organizerHistoryBtn", organizerShowHistory);
  const zoneSelect = document.getElementById("organizerZoneSelect");
  if (zoneSelect) {
    zoneSelect.addEventListener("change", (e) => {
      organizerSelectedZoneId = e.target.value || null;
      organizerHasReviewedPlan = false;
      const result = document.getElementById("organizerResult");
      if (result) result.innerHTML = "";
      organizerRefreshRules();
    });
  }

  // Lock screen
  bind("unlockBtn", tryUnlock);
  const lockPassword = document.getElementById("lockPassword");
  if (lockPassword) {
    lockPassword.addEventListener("keydown", (e) => {
      if (e.key === "Enter") tryUnlock();
    });
  }

  // Onboarding navigation: welcome → how-it-helps → permissions → password → ready.
  // Every step offers a way to ignore (skip), accept, or keep going.
  bind("onboardingIgnore", finishOnboarding);
  bind("onboardingStart", () => showOnboardingStep(1));
  bind("onboardingBack", () => showOnboardingStep(0));
  bind("onboardingDemoNext", () => showOnboardingStep(2));
  bind("onboardingBack2", () => showOnboardingStep(1));
  bind("onboardingGrant", onboardingGrant);
  bind("onboardingPermNext", () => showOnboardingStep(3));
  bind("onboardingBack3", () => showOnboardingStep(2));
  bind("onboardingSkipPassword", () => showOnboardingStep(4));
  bind("onboardingSetPassword", onboardingSetPassword);
  // Once a password exists the password step auto-advances, so route this
  // Back past it to the permissions step.
  bind("onboardingBack4", () =>
    showOnboardingStep(authStatus.configured ? ONBOARDING_PERM_STEP : ONBOARDING_PASSWORD_STEP),
  );
  bind("onboardingFinish", finishOnboarding);
  bind("onboardingSkip", finishOnboarding);

  const speedSelect = document.getElementById("speedSelect");
  if (speedSelect) speedSelect.addEventListener("change", (e) => setSpeed(parseFloat(e.target.value)));

  // Modal close / dynamically-injected suggestion buttons (event delegation,
  // since their markup is generated via innerHTML after the fact)
  document.body.addEventListener("click", (e) => {
    const closeTarget = e.target.closest("[data-close-modal]");
    if (closeTarget) {
      closeModal(closeTarget.dataset.closeModal);
      return;
    }

    const suggestionTarget = e.target.closest("[data-create-workflow-from-suggestion]");
    if (suggestionTarget) {
      createWorkflowFromSuggestion(suggestionTarget.dataset.workflowName);
      return;
    }

    const saveConfigTarget = e.target.closest("[data-save-config]");
    if (saveConfigTarget) {
      saveSettings();
    }
  });
}

// --- Signed auto-update (suggest -> user approves -> apply) -----------------
// Ghost never swaps itself out silently: it checks for a signed update, tells
// the user, and installs only when they click "Update now". The check is
// read-only and failures (no endpoint, unconfigured key) are swallowed so they
// can never disrupt the app.
async function checkForUpdatesOnLaunch() {
  if (!invoke) return; // static/dev mode: nothing to check against
  try {
    const info = await invoke("check_for_update");
    if (info) showUpdatePrompt(info);
  } catch (err) {
    console.warn("Update check skipped:", err);
  }
}

function showUpdatePrompt(info) {
  const container = document.getElementById("notifications");
  if (!container) return;

  const card = document.createElement("div");
  card.className = "notification notification--info";

  const text = document.createElement("p");
  text.className = "notification__text";
  text.textContent = `✓ Ghost ${info.version} is available (you have ${info.current_version}).`;
  card.appendChild(text);

  const actions = document.createElement("div");
  actions.className = "notification__actions";

  const updateBtn = document.createElement("button");
  updateBtn.type = "button";
  updateBtn.textContent = "Update now";
  updateBtn.addEventListener("click", () => installApprovedUpdate(updateBtn, text));

  const laterBtn = document.createElement("button");
  laterBtn.type = "button";
  laterBtn.textContent = "Later";
  laterBtn.addEventListener("click", () => card.remove());

  actions.appendChild(updateBtn);
  actions.appendChild(laterBtn);
  card.appendChild(actions);
  container.appendChild(card);
}

async function installApprovedUpdate(button, statusEl) {
  if (!invoke) return;
  button.disabled = true;
  statusEl.textContent = "Downloading and installing update… Ghost will restart.";
  try {
    // Verifies the signature against the embedded public key, installs, relaunches.
    await invoke("install_update");
  } catch (err) {
    button.disabled = false;
    statusEl.textContent = `Update failed: ${err}`;
    toastError(`Update failed: ${err}`);
  }
}

// The experimental tools panel drives commands that only exist when the backend
// is compiled with `--features experimental`. A stock build does not register
// them, so reveal the panel only when the backend confirms the surface is on.
// Fail closed: any error leaves the panel hidden rather than showing dead buttons.
async function initExperimentalPanel() {
  const panel = document.getElementById("experimentalPanel");
  if (!panel || !invoke) return;
  try {
    if (await invoke("is_experimental_enabled")) {
      panel.hidden = false;
    }
  } catch {
    // Older/stock build without the detection command: keep it hidden.
  }
}

window.addEventListener("DOMContentLoaded", () => {
  wireUpControls();
  updateRecordingUI();
  refreshPermissionBanner();
  initAuthGate(); // lock screen (if password set) or first-run walkthrough
  syncSpeedFromConfig();
  checkForUpdatesOnLaunch(); // signed, user-approved auto-update
  organizerInit(); // Ghost Organizer: load Zones and wire the trust pipeline
  initExperimentalPanel(); // reveal experimental tools only in experimental builds
});
