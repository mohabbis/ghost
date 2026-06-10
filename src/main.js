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

// Listen for ghost events from the backend
if (listen) {
  listen("ghost:event", (event) => {
    console.log("Ghost event captured:", event.payload);
    if (isRecording) {
      recordedEvents.push(event.payload);
      updateRecordingUI();
      addEventToTimeline(event.payload);
    }
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
  notification.className = "notification notification--proactive";
  if (kind === "error") {
    notification.style.borderColor = "#ef4444";
    notification.style.background = "rgba(239, 68, 68, 0.12)";
  }
  const icon = kind === "error" ? "⚠️" : "🦜";
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
        ${options.map((o) => `<button class="btn btn--ghost" data-dialog-option="${escapeHtml(o)}" style="justify-content:flex-start;text-align:left;">${escapeHtml(o)}</button>`).join("")}
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
    const timelineEl = document.getElementById("events-timeline");
    if (timelineEl) timelineEl.innerHTML = "";
    updateRecordingUI();
    showInsight("Recording your actions…");
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
    showInsight(`Captured ${recordedEvents.length} events. Ready to replay or save.`);
  } catch (error) {
    console.error("Failed to stop recording:", error);
  }
}

async function replayWorkflow() {
  if (!invoke) return notAvailable();

  try {
    isPlaying = true;
    updateRecordingUI();
    await invoke("replay_workflow", { events: recordedEvents });
    isPlaying = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to replay workflow:", error);
    isPlaying = false;
    updateRecordingUI();
  }
}

async function replayWithReliability() {
  if (!invoke) return notAvailable();

  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }

  try {
    // Retry behavior comes from Settings (replay.*) — no popups.
    const config = await invoke("get_config");
    const replay = config?.replay ?? {};

    isPlaying = true;
    updateRecordingUI();
    await invoke("replay_with_reliability", {
      events: recordedEvents,
      max_attempts: replay.max_retry_attempts ?? 3,
      backoff_ms: replay.retry_backoff_ms ?? 500,
      backoff_multiplier: replay.retry_backoff_multiplier ?? 2.0,
    });
    isPlaying = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to replay with reliability:", error);
    isPlaying = false;
    updateRecordingUI();
    toastError("Replay failed: " + error);
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

// ===== Workflow management =====

async function saveWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    toastError("No events recorded yet");
    return;
  }
  const name = await ghostPrompt("Name this workflow", "", "e.g. Friday timesheet");
  if (!name) return;

  try {
    await invoke("save_workflow", { name, events: recordedEvents });
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
    updateRecordingUI();
    refreshTimeline();
    showNotification(`Loaded "${name}" — ${recordedEvents.length} events.`);
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
    <h3>Workflow Analysis: ${analysis.workflow_name}</h3>
    <p><strong>Total Events:</strong> ${analysis.total_events}</p>
    <p><strong>Estimated Duration:</strong> ${analysis.estimated_duration_ms}ms</p>
    <p><strong>Reliability Score:</strong> ${(analysis.reliability_score * 100).toFixed(1)}%</p>
    <p><strong>Element Richness:</strong> ${(analysis.element_richness * 100).toFixed(1)}%</p>

    ${analysis.patterns.length > 0 ? `
    <h4>Detected Patterns</h4>
    <ul>
      ${analysis.patterns.map((p) => `<li>${p.description} (confidence: ${(p.confidence * 100).toFixed(1)}%)</li>`).join("")}
    </ul>
    ` : ""}

    ${analysis.suggested_optimizations.length > 0 ? `
    <h4>Suggested Optimizations</h4>
    <ul>
      ${analysis.suggested_optimizations.map((o) => `<li>${o.description}</li>`).join("")}
    </ul>
    ` : ""}

    <button data-close-modal="analysis-modal">Close</button>
  `;

  modal.style.display = "flex";
}

function closeModal(modalId = "analysis-modal") {
  const modal = document.getElementById(modalId);
  if (modal) modal.style.display = "none";
}

// ===== Settings =====

// Cache the full config so we can send a complete GhostConfig back to the
// backend (update_config deserializes the whole struct, not a partial patch).
let settingsConfig = null;

function escapeAttr(value) {
  return String(value ?? "").replace(/"/g, "&quot;");
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

  modal.style.display = "flex";
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
        description = `${kind} ${role}${name}${app}`;
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
  if (cancelBtn) cancelBtn.disabled = !isPlaying;
  if (pauseBtn) pauseBtn.disabled = !isPlaying || isPaused;
  if (resumeBtn) resumeBtn.disabled = !isPlaying || !isPaused;
}

// ===== Smart Observer mode =====

let observerUpdateInterval = null;

async function startSmartObserver() {
  if (!invoke) return notAvailable();

  try {
    await invoke("start_observer");
    showInsight("Smart Observer started — I'm learning your patterns…");
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
    showInsight("Smart Observer stopped.");
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
    const patternsFound = await invoke("observe_events", { events: recordedEvents, app_name: appName });
    showNotification(`Found ${patternsFound} learned patterns from <strong>${appName}</strong>!`);

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
    const insights = await invoke("generate_geek_insights", { events: recordedEvents, app_name: appName });
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
        <p><strong>${i + 1}. ${s.suggestion}</strong></p>
        <p style="font-size: 0.9rem; color: #9ca3af;">Suggested workflow: <code>${s.suggested_workflow_name}</code></p>
        <p style="font-size: 0.85rem;">Confidence: ${(s.confidence * 100).toFixed(1)}%</p>
        <button data-create-workflow-from-suggestion="${s.suggested_workflow_name}|${s.pattern_id}" style="margin-top: 8px; font-size: 0.85rem;">Create This Workflow</button>
      </div>
    `).join("")}
    <button data-close-modal="analysis-modal">Close</button>
  `;

  modal.style.display = "flex";
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
    <h3>🔧 Geek Mode: Technical Insights for ${appName}</h3>
    <div style="margin: 12px 0;">
      <h4 style="color: #8d7bff;">Performance Metrics</h4>
      <p>Total Duration: ${insights.performance_metrics.total_duration_ms}ms</p>
      <p>Avg Delay: ${insights.performance_metrics.avg_delay_ms.toFixed(2)}ms</p>
      ${insights.performance_metrics.bottleneck_events.length > 0 ? `
        <p>Bottleneck Events: ${insights.performance_metrics.bottleneck_events.join(", ")}</p>
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
            <td>${t.estimated_action}</td>
            <td>${t.delay_before_ms}ms</td>
          </tr>
        `).join("")}
        ${insights.event_timing_analysis.length > 10 ? `<tr><td colspan="3">... and ${insights.event_timing_analysis.length - 10} more</td></tr>` : ""}
      </table>
    </div>
    <button data-close-modal="analysis-modal">Close</button>
  `;

  modal.style.display = "flex";
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

    const success = await invoke("replay_with_visual_check", { events: recordedEvents, visual_checks: visualChecks });
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

async function createDataSource() {
  if (!invoke) return;

  const name = await ghostPrompt("Data source name");
  if (!name) return;

  const type = (await ghostPick("Data source type", ["environment", "csv", "json"])) || "environment";
  let path = null;
  if (type === "csv" || type === "json") path = await ghostPrompt("Path to data file");

  try {
    await invoke("create_data_source", { name, source_type: type, path });
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
    const variables = await invoke("load_variables", { data_source_name: name });
    showNotification(`Loaded ${Object.keys(variables).length} variables.`);
    console.log("Variables:", variables);
  } catch (error) {
    console.error("Failed to load variables:", error);
    toastError("Load failed: " + error);
  }
}

// ===== Wire up the UI =====

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

  bind("saveBtn", saveWorkflow);
  bind("saveAiBtn", saveWorkflowWithMetadata);
  bind("loadBtn", loadWorkflow);
  bind("analyzeBtn", analyzeWorkflow);
  bind("optimizeBtn", optimizeWorkflow);

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
      const [name] = suggestionTarget.dataset.createWorkflowFromSuggestion.split("|");
      createWorkflowFromSuggestion(name);
      return;
    }

    const saveConfigTarget = e.target.closest("[data-save-config]");
    if (saveConfigTarget) {
      saveSettings();
    }
  });
}

window.addEventListener("DOMContentLoaded", () => {
  wireUpControls();
  updateRecordingUI();
  refreshPermissionBanner();
  initAuthGate(); // lock screen (if password set) or first-run walkthrough
  syncSpeedFromConfig();
});
