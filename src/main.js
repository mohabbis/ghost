// Ghost desktop app — Tauri IPC integration, recording controls, workflow
// management, and Smart Observer. This is the real app UI
// (not the marketing site — that lives in public/).

import { CompressionReview } from "./compression-review.js";

const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};

// Inline line-icon helper for HTML built in JS — references the SVG sprite in
// index.html so dynamic UI uses the same icon set as the static markup (no emoji).
const icon = (id, cls = "i") => `<svg class="${cls}" aria-hidden="true"><use href="#${id}"/></svg>`;

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
let compressionReview = null;
/** Last Guard Desk scan payload (OCR or preset) — used by Auto-fill POS. */
let guardLastScanData = null;
/** Cached from `is_experimental_enabled` — gates observer IPC on stock builds. */
let experimentalEnabled = false;
let guardScanInProgress = false;

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
  const glyph =
    kind === "error" || kind === "warn"
      ? icon("i-alert")
      : icon("i-check-circle");
  notification.innerHTML = `<p class="notification__text">${glyph} ${escapeHtml(text)}</p>`;
  notificationsEl.appendChild(notification);

  const ttl = kind === "error" ? 8000 : kind === "warn" ? 6500 : 5000;
  setTimeout(() => notification.remove(), ttl);
}


/** Turn Tauri/IPC errors into readable toast text (avoid "[object Object]"). */
function formatInvokeError(err) {
  if (err == null || err === "") return "Unknown error";
  if (typeof err === "string") return err;
  if (typeof err === "number" || typeof err === "boolean") return String(err);
  if (err instanceof Error && err.message) return err.message;
  if (typeof err === "object") {
    if (typeof err.message === "string" && err.message.trim()) return err.message;
    if (typeof err.error === "string" && err.error.trim()) return err.error;
    try {
      const json = JSON.stringify(err);
      if (json && json !== "{}" && json !== "null") return json;
    } catch (_) {
      /* ignore */
    }
  }
  const s = String(err);
  return s === "[object Object]" ? "Unexpected error" : s;
}

const toastError = (text) => showNotification(typeof text === "string" ? text : formatInvokeError(text), "error");

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

let _permAutoPrompted = false;
/** After Grant Access, checks often stay false until Quit & Reopen — guide the user there. */
let _permAwaitingRelaunch = false;

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function nextMissingPermission({ accessibility, inputMonitoring }) {
  if (!accessibility) return "accessibility";
  if (!inputMonitoring) return "input_monitoring";
  return null;
}

function permissionBannerCopy({ accessibility, inputMonitoring }) {
  const ax = accessibility ? "Accessibility: on" : "Accessibility: off";
  const im = inputMonitoring ? "Input Monitoring: on" : "Input Monitoring: off";
  const status = `${ax} · ${im}.`;

  if (accessibility && inputMonitoring) {
    return status;
  }

  if (_permAwaitingRelaunch) {
    return `${status} If you already enabled Ghost in System Settings, click Quit & Reopen — macOS applies these grants only after relaunch (use the Ghost entry that matches this build).`;
  }

  const next =
    nextMissingPermission({ accessibility, inputMonitoring }) === "accessibility"
      ? "Accessibility"
      : "Input Monitoring";
  return `${status} Click Grant Access for ${next}, enable Ghost in that list, then Quit & Reopen. Organizer file filing works without them.`;
}

function updateGrantButtonLabel({ accessibility, inputMonitoring }) {
  const grant = document.getElementById("perm-grant");
  const onboardingGrant = document.getElementById("onboardingGrant");
  const next = nextMissingPermission({ accessibility, inputMonitoring });
  let label = "Grant Access";
  if (next === "accessibility") label = "Grant Accessibility";
  else if (next === "input_monitoring") label = "Grant Input Monitoring";
  else label = "Permissions OK";
  if (grant) grant.textContent = label;
  if (onboardingGrant) onboardingGrant.textContent = label;
}

async function refreshPermissionBanner() {
  if (!invoke) return;

  const banner = document.getElementById("perm-banner");
  if (!banner) return;

  try {
    const perms = await checkPermissions();
    const { accessibility, inputMonitoring } = perms;
    const allGranted = accessibility && inputMonitoring;
    banner.hidden = allGranted;
    updateMissionProgress({ permissionsGranted: allGranted });
    updateGrantButtonLabel(perms);

    if (allGranted) {
      _permAwaitingRelaunch = false;
      return;
    }

    const text = document.getElementById("perm-text");
    if (text) text.textContent = permissionBannerCopy(perms);

    const restart = document.getElementById("perm-restart");
    if (restart) {
      restart.classList.toggle("btn--primary", _permAwaitingRelaunch);
      restart.classList.toggle("btn--ghost", !_permAwaitingRelaunch);
    }

    // Auto-prompt once on startup — only the first missing permission so we
    // never overwrite Accessibility Settings with Input Monitoring a moment later.
    if (!_permAutoPrompted) {
      _permAutoPrompted = true;
      const next = nextMissingPermission(perms);
      if (next === "accessibility") {
        await invoke("request_accessibility").catch(() => {});
        _permAwaitingRelaunch = true;
      } else if (next === "input_monitoring") {
        await invoke("request_input_monitoring").catch(() => {});
        _permAwaitingRelaunch = true;
      }
      if (text) text.textContent = permissionBannerCopy(await checkPermissions());
      if (restart && _permAwaitingRelaunch) {
        restart.classList.add("btn--primary");
        restart.classList.remove("btn--ghost");
      }
    }
  } catch (error) {
    console.error("Failed to check permissions:", error);
  }
}

/** Request exactly one missing permission per click (never race two Settings panes). */
async function requestAccessibility() {
  if (!invoke) {
    notAvailable();
    return;
  }
  try {
    const before = await checkPermissions();
    const next = nextMissingPermission(before);

    if (!next) {
      _permAwaitingRelaunch = false;
      showNotification("Accessibility and Input Monitoring are both granted.");
      return;
    }

    if (next === "accessibility") {
      await invoke("request_accessibility");
      _permAwaitingRelaunch = true;
      showNotification(
        "Enable Ghost under Accessibility, then Quit & Reopen. We'll ask for Input Monitoring next if needed.",
      );
    } else {
      await invoke("request_input_monitoring");
      _permAwaitingRelaunch = true;
      showNotification(
        "Enable Ghost under Input Monitoring, then Quit & Reopen so macOS applies the grant.",
      );
    }

    await wait(400);
    const after = await checkPermissions();
    // Still false is normal until relaunch — don't treat it as failure.
    if (!after.accessibility || !after.inputMonitoring) {
      showInsight(
        "macOS usually keeps permissions “off” in the running app until Quit & Reopen — even after you flip the switch.",
      );
    }
  } catch (error) {
    console.error("Failed to request permissions:", error);
    showNotification(
      "Couldn't open permission settings. Open System Settings → Privacy & Security, enable Ghost, then Quit & Reopen.",
      "error",
    );
  } finally {
    await refreshPermissionBanner();
  }
}

// macOS only re-evaluates permission trust at launch, so after the user flips
// the switch the running process still reads "not granted". Relaunching applies
// it. See restart_app in src-tauri/src/commands/core.rs.
async function restartApp() {
  if (!invoke) {
    notAvailable();
    return;
  }
  try {
    await invoke("restart_app");
  } catch (error) {
    console.error("Failed to restart:", error);
    showNotification("Couldn't relaunch automatically — quit Ghost and open it again to apply the permission.", "error");
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
    toastError("Unlock failed: " + formatInvokeError(err));
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
    toastError("Could not lock: " + formatInvokeError(error));
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
  let accessibility = false;
  let inputMonitoring = false;
  try {
    ({ accessibility, inputMonitoring } = await checkPermissions());
  } catch (error) {
    console.error("Failed to check permissions:", error);
    return;
  }

  const granted = accessibility && inputMonitoring;
  const status = document.getElementById("onboardingPermStatus");
  const text = document.getElementById("onboardingPermStatusText");
  const next = document.getElementById("onboardingPermNext");
  const grant = document.getElementById("onboardingGrant");
  if (!status) return;

  status.dataset.granted = granted ? "true" : "false";
  updateGrantButtonLabel({ accessibility, inputMonitoring });

  if (text) {
    if (granted) {
      text.textContent = "Access granted";
    } else if (_permAwaitingRelaunch) {
      text.textContent = "Toggle is on? Click Quit & Reopen — macOS applies grants at launch";
    } else {
      const parts = [];
      parts.push(accessibility ? "Accessibility on" : "Accessibility off");
      parts.push(inputMonitoring ? "Input Monitoring on" : "Input Monitoring off");
      text.textContent = parts.join(" · ");
    }
  }

  // Once granted, make "Next" the obvious action and de-emphasize "Grant".
  if (granted) {
    stopPermPolling();
    _permAwaitingRelaunch = false;
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
    toastError("Could not start recording: " + formatInvokeError(error));
    showInsight("Recording blocked — check permissions above.");
  }
}

async function stopRecording() {
  if (!invoke) return notAvailable();

  try {
    await invoke("stop_recording");
    isRecording = false;
    updateRecordingUI();
    await refreshTimeline();
    showInsight(`Captured ${recordedEvents.length} event(s). Review the timeline, then replay or save.`);
    updateWorkflowHealth();
    runGhostGuardAudit({ quiet: true });
  } catch (error) {
    console.error("Failed to stop recording:", error);
    toastError("Could not stop recording: " + formatInvokeError(error));
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
  if (!invoke || recordedEvents.length === 0 || !experimentalEnabled) return;

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

/** Guard → policy plan → explicit approve (server one-shot token) before OS replay. */
async function confirmPolicyBeforeReplay(events) {
  if (!events || events.length === 0) return false;
  if (!(await confirmGuardBeforeReplay())) return false;

  let plan;
  try {
    plan = await invoke("routine_policy_plan", { events });
  } catch (error) {
    toastError("Policy plan failed: " + formatInvokeError(error));
    return false;
  }

  if (!plan?.can_proceed_with_approvals) {
    const denied = (plan?.steps || [])
      .filter((s) => s.decision?.decision === "deny")
      .map((s) => `step ${s.step_index + 1}: ${s.decision.reason || "denied"}`)
      .slice(0, 4)
      .join("; ");
    toastError(`Replay blocked by policy${denied ? `: ${denied}` : "."}`);
    showInsight("Policy denied one or more routine steps. Edit or re-record before replaying.");
    return false;
  }

  const confirmCount = plan.confirmation_count || 0;
  const summary =
    confirmCount > 0
      ? `Policy requires confirmation for ${confirmCount} OS-control step(s). Approve and replay?`
      : "Approve this routine policy plan and replay?";
  const ok = await ghostConfirm(summary, "Approve & Replay");
  if (!ok) {
    showInsight("Replay cancelled — approve the policy plan when ready.");
    return false;
  }

  try {
    await invoke("approve_routine_replay", { events });
  } catch (error) {
    toastError("Could not approve routine: " + formatInvokeError(error));
    return false;
  }
  return true;
}

async function replayWorkflow() {
  if (!invoke) return notAvailable();
  if (!(await confirmPolicyBeforeReplay(recordedEvents))) return;

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
    toastError("Replay failed: " + formatInvokeError(error));
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

  if (!(await confirmPolicyBeforeReplay(recordedEvents))) return;

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
    toastError("Replay failed: " + formatInvokeError(error));
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
    toastError("Could not cancel replay: " + formatInvokeError(error));
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
    toastError("Could not pause replay: " + formatInvokeError(error));
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
    toastError("Could not resume replay: " + formatInvokeError(error));
  }
}

async function setSpeed(factor) {
  if (!invoke) return;
  const speedSelect = document.getElementById("speedSelect");
  const prev = playbackSpeed;
  try {
    await invoke("set_playback_speed", { factor });
    playbackSpeed = factor;
  } catch (error) {
    console.error("Failed to set speed:", error);
    toastError("Could not set playback speed: " + formatInvokeError(error));
    if (speedSelect) speedSelect.value = String(prev);
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
    toastError("Inspect failed: " + formatInvokeError(error));
  }
}


// ===== Ghost Guard local audit =====

function severityIcon(severity) {
  const id = { low: "i-info", medium: "i-alert", high: "i-shield", critical: "i-siren" }[severity];
  return id ? icon(id, `i sev sev--${severity}`) : "•";
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
    if (!quiet) toastError("Ghost Guard audit failed: " + formatInvokeError(error));
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
    toastError("Failed to save workflow: " + formatInvokeError(error));
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
    await refreshTimeline();
    showNotification(`Loaded "${name}" — ${recordedEvents.length} events.`);
    runGhostGuardAudit({ quiet: true });
  } catch (error) {
    console.error("Failed to load workflow:", error);
    toastError("Failed to load workflow: " + formatInvokeError(error));
  }
}

async function deleteWorkflow() {
  if (!invoke) return;

  try {
    const names = await invoke("list_workflows");
    const name = await ghostPick("Delete a workflow", names);
    if (!name) return;

    const ok = await ghostConfirm(`Delete "${name}"? This cannot be undone.`, "Delete");
    if (!ok) return;

    await invoke("delete_workflow", { name });
    showNotification(`Deleted "${name}".`);
  } catch (error) {
    console.error("Failed to delete workflow:", error);
    toastError("Failed to delete workflow: " + formatInvokeError(error));
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
    toastError("Failed to analyze workflow: " + formatInvokeError(error));
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
    void refreshTimeline();
    showNotification(`Optimized: ${originalCount} events → ${optimized.length} events.`);
  } catch (error) {
    console.error("Failed to optimize workflow:", error);
    toastError("Failed to optimize workflow: " + formatInvokeError(error));
  }
}

function refreshTimeline() {
  return refreshCompressedTimeline();
}

function initCompressionReview() {
  if (!compressionReview) {
    compressionReview = new CompressionReview("events-timeline", invoke);
  }
}

async function refreshCompressedTimeline() {
  const timelineEl = document.getElementById("events-timeline");
  if (!timelineEl) return;

  if (!invoke || recordedEvents.length === 0) {
    timelineEl.innerHTML =
      '<p class="events-timeline__empty">Start a recording to see cleaned, human-readable steps here.</p>';
    updateWorkflowHealth();
    return;
  }

  initCompressionReview();
  try {
    await compressionReview.compress(recordedEvents);
  } catch (error) {
    console.error("Timeline compression failed:", error);
    timelineEl.innerHTML = "";
    recordedEvents.forEach((event) => addEventToTimeline(event));
    trimTimeline();
    toastError("Could not compress timeline — showing raw events.");
  }
  updateWorkflowHealth();
}

async function openWorkflowReview() {
  if (!invoke) return notAvailable();
  if (recordedEvents.length === 0) {
    toastError("Record or load a workflow first");
    return;
  }

  const modal = document.getElementById("review-modal");
  const container = document.getElementById("review-modal-compression");
  if (!modal || !container) return;

  const reviewBtn = document.getElementById("reviewBtn");
  if (reviewBtn) {
    reviewBtn.disabled = true;
    reviewBtn.textContent = "Reviewing…";
  }

  try {
    const panelReview = new CompressionReview("review-modal-compression", invoke);
    const report = await panelReview.compress(recordedEvents);
    showModal(modal);
    showInsight(
      `Reviewed ${report.compressed_step_count} step(s) — ${(report.reduction_ratio * 100).toFixed(0)}% fewer than raw events.`,
    );
  } catch (error) {
    console.error("Workflow review failed:", error);
    toastError("Review failed: " + formatInvokeError(error));
  } finally {
    if (reviewBtn) {
      reviewBtn.disabled = recordedEvents.length === 0;
      reviewBtn.textContent = "Review Steps";
    }
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
    void refreshTimeline();
    showNotification(`Generated ${events.length} events from your description.${providerNote}`);
    showInsight("Review the generated steps in the timeline, then Replay or Save.");
  } catch (error) {
    console.error("Failed to generate workflow:", error);
    toastError("Generate failed: " + formatInvokeError(error));
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
    toastError("Failed to save workflow: " + formatInvokeError(error));
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

    <button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button>
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

function initModalEscape() {
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    const openModal = [...document.querySelectorAll(".modal")].find(
      (m) => m.style.display === "flex",
    );
    if (!openModal) return;
    event.preventDefault();
    closeModal(openModal.id);
  });
}

// ===== What is Ghost? =====
// One canonical, always-available answer to "what is this, what does it do, and
// what will it never do." Kept honest and consistent with README.md / AGENTS.md
// so the product describes itself the same way everywhere. Static trusted copy.
function openAbout() {
  const modal = document.getElementById("about-modal");
  if (!modal) return;
  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>${icon("i-info")} What is Ghost?</h3>
    <p class="about-lead"><strong>Ghost turns repetitive file and desktop work into routines you approve before they run.</strong> It's a local-first app for macOS and Windows — your files, logs, and history stay on your machine.</p>

    <div class="about-section">
      <h4 class="about-h">How it works</h4>
      <div class="about-flow" aria-label="Plan, approve, act, audit, undo">
        <span>Plan</span><span aria-hidden="true">→</span>
        <span>You approve</span><span aria-hidden="true">→</span>
        <span>Ghost acts</span><span aria-hidden="true">→</span>
        <span>Audit log</span><span aria-hidden="true">→</span>
        <span>Undo</span>
      </div>
      <p class="panel__hint" style="margin-top:8px;">Nothing changes until you approve the exact plan. Every change is logged, and reversible changes can be undone.</p>
    </div>

    <div class="about-section">
      <h4 class="about-h">What it does today</h4>
      <ul class="about-list">
        <li><strong>Organize</strong> — pick a folder, preview every move and rename, approve, then undo if needed. Never deletes or overwrites silently.</li>
        <li><strong>Plan Filing</strong> — preview how software automation artifacts, finance reports, or student coursework would be sorted by type and period (from file names only), and estimate the time it saves.</li>
        <li><strong>Guard Desk</strong> — read a check or ID image with on-device OCR and run compliance checks locally; nothing is uploaded.</li>
        <li><strong>Record &amp; Replay</strong> — capture a task once, review each step, and replay only the steps you approved.</li>
      </ul>
    </div>

    <div class="about-section">
      <h4 class="about-h">What Ghost can't do yet</h4>
      <ul class="about-list about-list--cant">
        <li>Run unattended or while you're away — it's built for explicit, interruptible work.</li>
        <li>Replay flawlessly across every app — when it can't find a control it falls back to coordinates, which can miss if the UI moves.</li>
        <li>Ship as a fully signed and notarized installer — current builds are an early preview.</li>
      </ul>
    </div>

    <div class="about-section">
      <h4 class="about-h">What Ghost will never do</h4>
      <ul class="about-list about-list--never">
        <li>Use your camera or microphone.</li>
        <li>Watch your screen in the background or observe you without an explicit, visible action.</li>
        <li>Read your email, browser tabs, or documents unless you hand it a specific file or action.</li>
        <li>Delete, overwrite, upload, or send anything silently.</li>
        <li>Let AI act on its own — AI may only suggest; deterministic code executes only what you approved.</li>
        <li>Store your data in the cloud or make network calls you didn't ask for.</li>
      </ul>
    </div>

    <p class="panel__hint">In one line: Ghost is boring, inspectable, and reversible — automation you can trust because you approve it.</p>

    <div style="display:flex; gap:8px; margin-top:16px;">
      <button class="btn btn--primary btn--small" data-close-modal="about-modal">Got it</button>
    </div>`;

  showModal(modal);
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

  let account = { signed_in: false };
  let intelStatus = { default_provider: "disabled", providers: [] };
  let powerBiStatus = { active: false };
  let fabricStatus = { active: false };
  let googleCloudStatus = { active: false };
  let mcpPairingStatus = { enabled: false };
  let mcpHttpStatus = { running: false };
  let mcpRelayStatus = { connected: false };
  let fabricWebhookStatus = { configured: false };
  try {
    account = await invoke("account_status");
  } catch (error) {
    console.error("Failed to load account status:", error);
  }
  try {
    mcpPairingStatus = await invoke("mcp_pairing_status");
  } catch (error) {
    console.error("Failed to load MCP pairing status:", error);
  }
  if (experimentalEnabled) {
    try {
      intelStatus = await invoke("intelligence_provider_status");
    } catch (error) {
      console.error("Failed to load intelligence provider status:", error);
    }
    try {
      powerBiStatus = await invoke("power_bi_grant_status");
    } catch (error) {
      console.error("Failed to load Power BI grant status:", error);
    }
    try {
      fabricStatus = await invoke("fabric_grant_status");
    } catch (error) {
      console.error("Failed to load Fabric grant status:", error);
    }
    try {
      googleCloudStatus = await invoke("google_grant_status");
    } catch (error) {
      console.error("Failed to load Google Cloud grant status:", error);
    }
    try {
      mcpHttpStatus = await invoke("mcp_http_server_status");
    } catch (error) {
      console.error("Failed to load MCP HTTP status:", error);
    }
    try {
      mcpRelayStatus = await invoke("mcp_relay_status");
    } catch (error) {
      console.error("Failed to load MCP relay status:", error);
    }
    try {
      fabricWebhookStatus = await invoke("fabric_webhook_status");
    } catch (error) {
      console.error("Failed to load Fabric webhook status:", error);
    }
  }

  const modal = document.getElementById("settings-modal");
  if (!modal) return;
  const content = modal.querySelector(".modal-content");
  if (!content) return;

  const { replay, ai } = settingsConfig;
  const intelligence = settingsConfig.intelligence || {
    default_provider: "disabled",
    openai: { model: "gpt-4o-mini", api_base: null, timeout_seconds: 60, max_input_bytes: 32768 },
    anthropic: { model: "claude-sonnet-4-20250514", timeout_seconds: 60, max_input_bytes: 32768 },
    local: {
      backend: "ollama",
      base_url: "http://127.0.0.1:11434/v1",
      model: "llama3.2",
      timeout_seconds: 60,
      max_input_bytes: 32768,
      allow_network_lan: false,
    },
    routing: { allow_fallback: false, local_only_for_sensitive_data: true, maximum_remote_payload_bytes: 32768 },
  };
  // `audit` may be absent in configs written before the retention feature.
  const audit = settingsConfig.audit || {};
  const providers = ["local", "openai", "anthropic", "local_model"];
  const intelProviders = [
    "disabled",
    "openai",
    "anthropic",
    "local_ollama",
    "local_lm_studio",
    "local_openai_compatible",
  ];
  const fieldStyle =
    "width: 100%; margin: 4px 0 12px; padding: 6px 8px; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; color: var(--text);";

  const intelStatusLine = (id) => {
    const entry = (intelStatus.providers || []).find((p) => p.id === id);
    if (!entry) return "Status unknown";
    const health =
      typeof entry.health === "string"
        ? entry.health
        : entry.health?.degraded?.reason
          ? `degraded: ${entry.health.degraded.reason}`
          : entry.health?.unavailable?.reason
            ? `unavailable: ${entry.health.unavailable.reason}`
            : "unknown";
    return entry.configured ? `configured · ${health}` : `no API key · ${health}`;
  };

  content.innerHTML = `
    <h3>${icon("i-gear")} Settings</h3>

    <h4 style="color: #0e8f78; margin-bottom: 4px;">Account</h4>
    ${
      account.signed_in
        ? `<p class="panel__hint" style="margin: 4px 0 8px;">Signed in as <strong>${escapeAttr(account.name || account.email)}</strong> (${escapeAttr(account.email)}) via ${escapeAttr(account.provider)}.</p>
           <button class="btn btn--ghost btn--small" data-account-sign-out>Sign out</button>`
        : `<p class="panel__hint" style="margin: 4px 0 8px;">Sign in to connect Ghost with your Microsoft or Google identity. This links who you are — it does not move your workflow or Organizer data anywhere.</p>
           <div style="display: flex; gap: 8px; flex-wrap: wrap;">
             <button class="btn btn--ghost btn--small" data-account-sign-in="google" ${account.google_sign_in_available ? "" : "disabled"}>Sign in with Google</button>
             <button class="btn btn--ghost btn--small" data-account-sign-in="microsoft" ${account.microsoft_sign_in_available ? "" : "disabled"}>Sign in with Microsoft</button>
           </div>`
    }
    <p class="panel__hint" id="account-status-note" style="margin: 4px 0 12px;">${
      account.signed_in
        ? ""
        : account.google_sign_in_available && !account.microsoft_sign_in_available
          ? "Google sign-in is ready. Microsoft requires an Entra app client ID in config."
          : !account.google_sign_in_available && account.microsoft_sign_in_available
            ? "Microsoft sign-in is ready. Google client ID is not configured."
            : !account.google_sign_in_available && !account.microsoft_sign_in_available
              ? "No sign-in providers are configured yet."
              : ""
    }</p>

    ${
      experimentalEnabled
        ? `<h4 style="color: #0e8f78; margin: 12px 0 4px;">AI Providers (Organizer planning)</h4>
           <p class="panel__hint" style="margin: 4px 0 8px;">Suggestion-only — models propose categories and rules; Ghost plans, you approve, deterministic code executes. Only zone-relative file metadata is sent by default.</p>
           <label>Default intelligence provider
             <select id="cfg-intel-default" style="${fieldStyle}">
               ${intelProviders
                 .map(
                   (p) =>
                     `<option value="${p}" ${p === (intelligence.default_provider || "disabled") ? "selected" : ""}>${p}</option>`,
                 )
                 .join("")}
             </select>
           </label>
           <label>OpenAI model
             <input id="cfg-intel-openai-model" type="text" value="${escapeAttr(intelligence.openai?.model ?? "gpt-4o-mini")}" style="${fieldStyle}">
           </label>
           <p class="panel__hint" style="margin: 0 0 4px;">OpenAI · ${escapeAttr(intelStatusLine("openai"))}</p>
           <label>OpenAI API key (stored encrypted locally; leave blank to keep current)
             <input id="cfg-intel-openai-key" type="password" autocomplete="off" placeholder="sk-…" style="${fieldStyle}">
           </label>
           <button class="btn btn--ghost btn--small" type="button" data-intel-test="openai">Test OpenAI connection</button>
           <label style="margin-top: 12px;">Anthropic model
             <input id="cfg-intel-anthropic-model" type="text" value="${escapeAttr(intelligence.anthropic?.model ?? "claude-sonnet-4-20250514")}" style="${fieldStyle}">
           </label>
           <p class="panel__hint" style="margin: 0 0 4px;">Anthropic · ${escapeAttr(intelStatusLine("anthropic"))}</p>
           <label>Anthropic API key (stored encrypted locally; leave blank to keep current)
             <input id="cfg-intel-anthropic-key" type="password" autocomplete="off" placeholder="sk-ant-…" style="${fieldStyle}">
           </label>
           <button class="btn btn--ghost btn--small" type="button" data-intel-test="anthropic">Test Anthropic connection</button>
           <h5 style="margin: 16px 0 4px;">Local models</h5>
           <p class="panel__hint" style="margin: 4px 0 8px;">Ollama, LM Studio, or any OpenAI-compatible localhost endpoint. Locality is not permission — suggestions still require your review before any file moves.</p>
           <label>Local backend
             <select id="cfg-intel-local-backend" style="${fieldStyle}">
               ${["ollama", "lm_studio", "openai_compatible"]
                 .map(
                   (b) =>
                     `<option value="${b}" ${b === (intelligence.local?.backend || "ollama") ? "selected" : ""}>${b}</option>`,
                 )
                 .join("")}
             </select>
           </label>
           <label>Local base URL
             <input id="cfg-intel-local-base-url" type="text" value="${escapeAttr(intelligence.local?.base_url ?? "http://127.0.0.1:11434/v1")}" style="${fieldStyle}">
           </label>
           <label>Local model
             <input id="cfg-intel-local-model" type="text" value="${escapeAttr(intelligence.local?.model ?? "llama3.2")}" style="${fieldStyle}">
           </label>
           <label style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
             <input id="cfg-intel-local-allow-lan" type="checkbox" ${intelligence.local?.allow_network_lan ? "checked" : ""}>
             Allow non-localhost endpoints (LAN)
           </label>
           <p class="panel__hint" style="margin: 0 0 4px;">Local · ${escapeAttr(intelStatusLine("local_ollama"))}</p>
           <button class="btn btn--ghost btn--small" type="button" data-intel-discover-local>Discover local runtimes</button>
           <pre id="intel-local-discover" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>
           <button class="btn btn--ghost btn--small" type="button" data-intel-test="local_ollama">Test local connection</button>
           <p class="panel__hint" id="intel-test-note" style="margin: 8px 0 12px;"></p>`
        : ""
    }

    ${
      experimentalEnabled
        ? `<h4 style="color: #0e8f78; margin: 12px 0 4px;">Power BI Export</h4>
           <p class="panel__hint" style="margin: 4px 0 8px;">Exports a summary of Organizer run history to a push dataset in your own Power BI "My workspace." Requires Microsoft sign-in first, then a separate, revocable Power BI consent. Always preview before pushing.</p>
           ${
             powerBiStatus.active
               ? `<p class="panel__hint" style="margin: 4px 0 8px;">Power BI connected${powerBiStatus.granted_at ? ` (granted ${escapeAttr(new Date(powerBiStatus.granted_at).toLocaleDateString())})` : ""}.</p>
                  <div style="display: flex; gap: 8px; margin-bottom: 8px;">
                    <button class="btn btn--ghost btn--small" type="button" data-power-bi-preview>Preview export</button>
                    <button class="btn btn--ghost btn--small" type="button" data-power-bi-push disabled>Push to Power BI</button>
                    <button class="btn btn--ghost btn--small" type="button" data-power-bi-revoke>Disconnect</button>
                  </div>
                  <pre id="power-bi-preview" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>`
               : `<button class="btn btn--ghost btn--small" type="button" data-power-bi-connect ${account.signed_in ? "" : "disabled"}>Connect Power BI</button>
                  ${!account.signed_in ? '<p class="panel__hint" style="margin: 4px 0 8px;">Sign in with Microsoft above first.</p>' : ""}`
           }
           <p class="panel__hint" id="power-bi-note" style="margin: 8px 0 12px;"></p>

           <h4 style="color: #0e8f78; margin: 12px 0 4px;">Microsoft Fabric</h4>
           <p class="panel__hint" style="margin: 4px 0 8px;">List workspaces, pick from the dropdowns, preview, then push JSON export files into the lakehouse Files folder.</p>
           ${
             fabricStatus.active
               ? `<p class="panel__hint" style="margin: 4px 0 8px;">Fabric connected${fabricStatus.granted_at ? ` (granted ${escapeAttr(new Date(fabricStatus.granted_at).toLocaleDateString())})` : ""}.</p>
                  <label>Workspace
                    <select id="fabric-workspace-id" style="${fieldStyle}">
                      <option value="">Select workspace…</option>
                    </select>
                  </label>
                  <label>Lakehouse
                    <select id="fabric-lakehouse-id" style="${fieldStyle}" disabled>
                      <option value="">Select lakehouse…</option>
                    </select>
                  </label>
                  <div style="display: flex; gap: 8px; margin-bottom: 8px; flex-wrap: wrap;">
                    <button class="btn btn--ghost btn--small" type="button" data-fabric-list-workspaces>Refresh workspaces</button>
                    <button class="btn btn--ghost btn--small" type="button" data-fabric-list-lakehouses>Refresh lakehouses</button>
                    <button class="btn btn--ghost btn--small" type="button" data-fabric-preview>Preview export</button>
                    <button class="btn btn--ghost btn--small" type="button" data-fabric-push disabled>Push to lakehouse</button>
                    <button class="btn btn--ghost btn--small" type="button" data-fabric-revoke>Disconnect</button>
                  </div>
                  <pre id="fabric-workspaces" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>
                  <pre id="fabric-lakehouses" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>
                  <pre id="fabric-preview" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>`
               : `<button class="btn btn--ghost btn--small" type="button" data-fabric-connect ${account.signed_in ? "" : "disabled"}>Connect Fabric</button>
                  ${!account.signed_in ? '<p class="panel__hint" style="margin: 4px 0 8px;">Sign in with Microsoft above first.</p>' : ""}`
           }
           <p class="panel__hint" id="fabric-note" style="margin: 8px 0 12px;"></p>

           <h4 style="color: #0e8f78; margin: 12px 0 4px;">Fabric inbound webhook</h4>
           <p class="panel__hint" style="margin: 4px 0 8px;">When the MCP HTTP server is running, POST to <code>${escapeAttr(fabricWebhookStatus.endpoint_path || "/fabric/webhook")}</code> with header <code>X-Ghost-Webhook-Secret</code>. Intents surface in Organizer — nothing auto-executes.</p>
           <p class="panel__hint" style="margin: 4px 0 8px;">Secret: ${fabricWebhookStatus.configured ? `configured (${escapeAttr(fabricWebhookStatus.secret_hint || "set")})` : "not set"}</p>
           <button class="btn btn--ghost btn--small" type="button" data-fabric-webhook-secret>${fabricWebhookStatus.configured ? "Rotate webhook secret" : "Generate webhook secret"}</button>
           <pre id="fabric-webhook-secret" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>

           <h4 style="color: #0e8f78; margin: 12px 0 4px;">Google Cloud Storage</h4>
           <p class="panel__hint" style="margin: 4px 0 8px;">Export Organizer audit history to a GCS bucket you choose. Requires Google sign-in, then a separate Cloud Storage grant.</p>
           ${
             googleCloudStatus.active
               ? `<p class="panel__hint" style="margin: 4px 0 8px;">Google Cloud connected${googleCloudStatus.granted_at ? ` (granted ${escapeAttr(new Date(googleCloudStatus.granted_at).toLocaleDateString())})` : ""}${googleCloudStatus.bound_bucket ? ` · bound bucket <strong>${escapeHtml(googleCloudStatus.bound_bucket)}</strong>` : ""}.</p>
                  <label>GCP project ID
                    <input id="google-project-id" type="text" placeholder="my-gcp-project" style="${fieldStyle}">
                  </label>
                  <label>Bucket
                    <select id="google-bucket" style="${fieldStyle}">
                      <option value="">Select bucket…</option>
                      ${googleCloudStatus.bound_bucket ? `<option value="${escapeAttr(googleCloudStatus.bound_bucket)}" selected>${escapeHtml(googleCloudStatus.bound_bucket)}</option>` : ""}
                    </select>
                  </label>
                  <div style="display: flex; gap: 8px; margin-bottom: 8px; flex-wrap: wrap;">
                    <button class="btn btn--ghost btn--small" type="button" data-google-list-buckets>List buckets</button>
                    <button class="btn btn--ghost btn--small" type="button" data-google-bind-bucket>Bind export bucket</button>
                    <button class="btn btn--ghost btn--small" type="button" data-google-preview>Preview export</button>
                    <button class="btn btn--ghost btn--small" type="button" data-google-push disabled>Push to bucket</button>
                    <button class="btn btn--ghost btn--small" type="button" data-google-revoke>Disconnect</button>
                  </div>
                  <pre id="google-preview" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>`
               : `<button class="btn btn--ghost btn--small" type="button" data-google-connect ${account.signed_in && account.provider === "google" ? "" : "disabled"}>Connect Google Cloud</button>
                  ${account.signed_in && account.provider === "google" ? "" : '<p class="panel__hint" style="margin: 4px 0 8px;">Sign in with Google above first.</p>'}`
           }
           <p class="panel__hint" id="google-note" style="margin: 8px 0 12px;"></p>`
        : ""
    }

    <h4 style="color: #0e8f78; margin: 12px 0 4px;">MCP access</h4>
    <p class="panel__hint" style="margin: 4px 0 8px;">Local MCP clients run <code>ghost mcp serve</code> (stdio) or start the HTTP server below. Pairing codes apply to MCP routes.</p>
    <p class="panel__hint" style="margin: 4px 0 8px;">Pairing: ${mcpPairingStatus.enabled ? `enabled (${escapeAttr(mcpPairingStatus.code_hint || "active")})` : "disabled — any local client can connect"}</p>
    <div style="display: flex; gap: 8px; margin-bottom: 8px; flex-wrap: wrap;">
      <button class="btn btn--ghost btn--small" type="button" data-mcp-enable-pairing>Enable pairing code</button>
      <button class="btn btn--ghost btn--small" type="button" data-mcp-disable-pairing ${mcpPairingStatus.enabled ? "" : "disabled"}>Disable pairing</button>
    </div>
    ${
      experimentalEnabled
        ? `<p class="panel__hint" style="margin: 8px 0 4px;">HTTP server: ${mcpHttpStatus.running ? `${mcpHttpStatus.tls_enabled ? "https" : "http"}://${escapeAttr(mcpHttpStatus.bind_host)}:${escapeAttr(mcpHttpStatus.port)}${mcpHttpStatus.lan_exposed ? " (LAN)" : ""}` : "stopped"}</p>
           <label>Port
             <input id="mcp-http-port" type="number" min="1024" max="65535" value="${escapeAttr(mcpHttpStatus.port || 8787)}" style="${fieldStyle}">
           </label>
           <label>Bearer token (required for LAN)
             <input id="mcp-http-bearer" type="password" placeholder="optional on localhost" style="${fieldStyle}">
           </label>
           <label>TLS certificate (PEM path, optional)
             <input id="mcp-http-tls-cert" type="text" placeholder="/path/to/ghost-mcp.crt" style="${fieldStyle}">
           </label>
           <label>TLS private key (PEM path, optional)
             <input id="mcp-http-tls-key" type="text" placeholder="/path/to/ghost-mcp.key" style="${fieldStyle}">
           </label>
           <label style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
             <input id="mcp-http-lan" type="checkbox">
             Expose on LAN (0.0.0.0) — higher risk; prefer TLS cert/key above
           </label>
           <div style="display: flex; gap: 8px; margin-bottom: 8px; flex-wrap: wrap;">
             <button class="btn btn--ghost btn--small" type="button" data-mcp-http-start ${mcpHttpStatus.running ? "disabled" : ""}>Start HTTP server</button>
             <button class="btn btn--ghost btn--small" type="button" data-mcp-http-stop ${mcpHttpStatus.running ? "" : "disabled"}>Stop HTTP server</button>
           </div>
           <p class="panel__hint" style="margin: 8px 0 4px;">Cloud relay: ${mcpRelayStatus.connected ? `connected to ${escapeAttr(mcpRelayStatus.relay_url || "")}` : "disconnected"}</p>
           <label>Relay URL (HTTPS)
             <input id="mcp-relay-url" type="url" placeholder="https://relay.example.com" value="${escapeAttr(mcpRelayStatus.relay_url || "")}" style="${fieldStyle}">
           </label>
           <label>Device ID
             <input id="mcp-relay-device-id" type="text" placeholder="my-laptop" value="${escapeAttr(mcpRelayStatus.device_id || "")}" style="${fieldStyle}">
           </label>
           <label>Relay token
             <input id="mcp-relay-token" type="password" placeholder="Bearer token for relay" style="${fieldStyle}">
           </label>
           <div style="display: flex; gap: 8px; margin-bottom: 8px; flex-wrap: wrap;">
             <button class="btn btn--ghost btn--small" type="button" data-mcp-relay-start ${mcpRelayStatus.connected ? "disabled" : ""}>Connect relay</button>
             <button class="btn btn--ghost btn--small" type="button" data-mcp-relay-stop ${mcpRelayStatus.connected ? "" : "disabled"}>Disconnect relay</button>
           </div>
           <p class="panel__hint" style="margin: 0 0 8px;">See <code>docs/mcp-relay.md</code> for the reference relay server.</p>`
        : ""
    }
    <pre id="mcp-pairing-code" class="panel__hint" style="margin: 4px 0 8px; white-space: pre-wrap; display: none;"></pre>
    <p class="panel__hint" id="mcp-pairing-note" style="margin: 8px 0 12px;"></p>

    <h4 style="color: #0e8f78; margin: 12px 0 4px;">Replay</h4>
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

    <h4 style="color: #0e8f78; margin: 12px 0 4px;">AI</h4>
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
    <label>Local model path (.gguf, for the "local_model" provider)
      <input id="cfg-ai-local-model-path" type="text" placeholder="/path/to/model.gguf"
             value="${escapeAttr(ai.local_model_path ?? "")}" style="${fieldStyle}">
    </label>
    <label>Local tokenizer path (tokenizer.json)
      <input id="cfg-ai-local-tokenizer-path" type="text" placeholder="/path/to/tokenizer.json"
             value="${escapeAttr(ai.local_tokenizer_path ?? "")}" style="${fieldStyle}">
    </label>
    <p class="panel__hint" style="margin: 4px 0 12px;">Runs entirely on device — nothing is uploaded. Requires a build with the experimental feature; otherwise "local_model" falls back to the built-in heuristics. Ghost does not ship or download model weights.</p>

    <h4 style="color: #0e8f78; margin: 12px 0 4px;">Audit retention</h4>
    <label>Keep only the most recent N runs
      <input id="cfg-audit-keep-last" type="number" step="1" min="1" placeholder="keep all"
             value="${escapeAttr(audit.retention_keep_last ?? "")}" style="${fieldStyle}">
    </label>
    <label>Delete runs older than N days
      <input id="cfg-audit-keep-days" type="number" step="1" min="1" placeholder="keep all"
             value="${escapeAttr(audit.retention_keep_days ?? "")}" style="${fieldStyle}">
    </label>
    <p class="panel__hint" style="margin: 4px 0 12px;">Leave blank to keep all audit history (the default). Pruning deletes your own past runs — sealed history you remove can't be recovered.</p>

    <p class="panel__hint" id="settings-about" style="margin: 16px 0 8px;">Ghost · local-first preview</p>
    <div style="display: flex; gap: 8px; margin-top: 8px;">
      <button class="btn btn--primary btn--small" data-save-config>Save</button>
      <button class="btn btn--ghost btn--small" data-close-modal="settings-modal">Cancel</button>
    </div>
  `;

  showModal(modal);

  if (experimentalEnabled && fabricStatus.active) {
    void listFabricWorkspaces();
  }

  // Best-effort version label (Tauri 2 global API). Never blocks settings.
  try {
    const ver = await window.__TAURI__?.app?.getVersion?.();
    const about = document.getElementById("settings-about");
    if (about && ver) about.textContent = `Ghost v${ver} · local-first preview`;
  } catch (_) {
    /* ignore */
  }
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
  const localModelPath = document.getElementById("cfg-ai-local-model-path")?.value?.trim();
  settingsConfig.ai.local_model_path = localModelPath ? localModelPath : null;
  const localTokenizerPath = document.getElementById("cfg-ai-local-tokenizer-path")?.value?.trim();
  settingsConfig.ai.local_tokenizer_path = localTokenizerPath ? localTokenizerPath : null;

  // Audit retention: a blank field means "keep all" (null); a positive integer
  // sets the bound. Anything invalid falls back to null rather than 0, which the
  // backend rejects (0 would wipe history on the next run).
  const posIntOrNull = (id) => {
    const raw = document.getElementById(id)?.value?.trim();
    if (!raw) return null;
    const n = Math.round(parseFloat(raw));
    return Number.isFinite(n) && n >= 1 ? n : null;
  };
  if (!settingsConfig.audit) settingsConfig.audit = {};
  settingsConfig.audit.retention_keep_last = posIntOrNull("cfg-audit-keep-last");
  settingsConfig.audit.retention_keep_days = posIntOrNull("cfg-audit-keep-days");

  if (!settingsConfig.intelligence) {
    settingsConfig.intelligence = {
      default_provider: "disabled",
      openai: { model: "gpt-4o-mini", timeout_seconds: 60, max_input_bytes: 32768 },
      anthropic: { model: "claude-sonnet-4-20250514", timeout_seconds: 60, max_input_bytes: 32768 },
      routing: { allow_fallback: false, local_only_for_sensitive_data: true, maximum_remote_payload_bytes: 32768 },
    };
  }
  settingsConfig.intelligence.default_provider =
    document.getElementById("cfg-intel-default")?.value || "disabled";
  if (!settingsConfig.intelligence.openai) settingsConfig.intelligence.openai = {};
  if (!settingsConfig.intelligence.anthropic) settingsConfig.intelligence.anthropic = {};
  settingsConfig.intelligence.openai.model =
    document.getElementById("cfg-intel-openai-model")?.value?.trim() ||
    settingsConfig.intelligence.openai.model;
  settingsConfig.intelligence.anthropic.model =
    document.getElementById("cfg-intel-anthropic-model")?.value?.trim() ||
    settingsConfig.intelligence.anthropic.model;
  if (!settingsConfig.intelligence.local) settingsConfig.intelligence.local = {};
  settingsConfig.intelligence.local.backend =
    document.getElementById("cfg-intel-local-backend")?.value ||
    settingsConfig.intelligence.local.backend ||
    "ollama";
  settingsConfig.intelligence.local.base_url =
    document.getElementById("cfg-intel-local-base-url")?.value?.trim() ||
    settingsConfig.intelligence.local.base_url;
  settingsConfig.intelligence.local.model =
    document.getElementById("cfg-intel-local-model")?.value?.trim() ||
    settingsConfig.intelligence.local.model;
  settingsConfig.intelligence.local.allow_network_lan =
    !!document.getElementById("cfg-intel-local-allow-lan")?.checked;

  const openaiKey = document.getElementById("cfg-intel-openai-key")?.value?.trim();
  const anthropicKey = document.getElementById("cfg-intel-anthropic-key")?.value?.trim();

  const saveBtn = document.querySelector("[data-save-config]");
  if (saveBtn?.disabled) return;
  if (saveBtn) saveBtn.disabled = true;

  try {
    if (experimentalEnabled) {
      if (openaiKey) {
        await invoke("intelligence_set_api_key", { provider: "openai", apiKey: openaiKey });
      }
      if (anthropicKey) {
        await invoke("intelligence_set_api_key", { provider: "anthropic", apiKey: anthropicKey });
      }
    }
    await invoke("update_config", { config: settingsConfig });
    // Reflect the new default speed in the picker and live state.
    playbackSpeed = settingsConfig.replay.default_speed;
    const speedSelect = document.getElementById("speedSelect");
    if (speedSelect) speedSelect.value = String(playbackSpeed);
    closeModal("settings-modal");
    showNotification("Settings saved.");
  } catch (error) {
    console.error("Failed to save config:", error);
    toastError(`Could not save settings: ${formatInvokeError(error)}`);
  } finally {
    if (saveBtn) saveBtn.disabled = false;
  }
}

// --- Account sign-in (Microsoft/Google OAuth) -------------------------------
// Identity only: links who the user is, for a future stack integration
// (Fabric/Power BI, Google Cloud, AI-assistant connectors) to request access
// under. Does not gate the app and does not move workflow/organizer data.
async function signOutAccount() {
  if (!invoke) return notAvailable();
  try {
    await invoke("account_sign_out");
    showNotification("Signed out.");
    openSettings();
  } catch (error) {
    toastError(`Sign-out failed: ${formatInvokeError(error)}`);
  }
}

async function testIntelligenceProvider(provider) {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("intel-test-note");
  if (note) note.textContent = `Testing ${provider}…`;
  try {
    const result = await invoke("intelligence_test_provider", { provider });
    const health =
      typeof result.health === "string"
        ? result.health
        : result.health?.degraded?.reason ||
          result.health?.unavailable?.reason ||
          JSON.stringify(result.health);
    if (note) note.textContent = `${provider}: ${health}`;
    showNotification(`${provider} test: ${health}`);
  } catch (error) {
    if (note) note.textContent = `${provider} test failed.`;
    toastError(formatInvokeError(error));
  }
}

async function discoverLocalRuntimes() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const pre = document.getElementById("intel-local-discover");
  if (pre) {
    pre.style.display = "block";
    pre.textContent = "Scanning localhost for Ollama and LM Studio…";
  }
  try {
    const found = await invoke("intelligence_discover_local");
    if (!found?.length) {
      if (pre) pre.textContent = "No local runtimes found on 127.0.0.1 (Ollama :11434, LM Studio :1234).";
      return;
    }
    const lines = found.map(
      (r) =>
        `${r.backend} @ ${r.base_url}\n  models: ${(r.models || []).slice(0, 5).join(", ")}${(r.models || []).length > 5 ? "…" : ""}`,
    );
    if (pre) pre.textContent = lines.join("\n\n");
    const first = found[0];
    const backendEl = document.getElementById("cfg-intel-local-backend");
    const baseEl = document.getElementById("cfg-intel-local-base-url");
    const modelEl = document.getElementById("cfg-intel-local-model");
    if (backendEl && first.backend) backendEl.value = first.backend;
    if (baseEl && first.base_url) baseEl.value = first.base_url;
    if (modelEl && first.models?.[0]) modelEl.value = first.models[0];
    showNotification(`Found ${found.length} local runtime(s).`);
  } catch (error) {
    if (pre) pre.textContent = `Discovery failed: ${formatInvokeError(error)}`;
    else toastError(formatInvokeError(error));
  }
}

// --- Power BI audit export (grant flow + preview + push) --------------------
// Experimental. Requires Microsoft sign-in first, then a separate, revocable
// Power BI consent. Always preview before pushing — the push button only
// enables after a preview has been shown for the current session.
let powerBiPreviewShown = false;
let fabricPreviewShown = false;
let googlePreviewShown = false;

async function connectPowerBi() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("power-bi-note");
  if (note) note.textContent = "Opening your browser to connect Power BI…";
  try {
    await invoke("power_bi_request_grant");
    showNotification("Power BI connected.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Connect failed: ${formatInvokeError(error)}`;
    else toastError(`Power BI connect failed: ${formatInvokeError(error)}`);
  }
}

async function previewPowerBiExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("power-bi-note");
  const pre = document.getElementById("power-bi-preview");
  try {
    const payload = await invoke("power_bi_export_preview", { sinceDays: null });
    if (pre) {
      pre.style.display = "block";
      pre.textContent = `${payload.runs.length} run(s), ${payload.actions.length} action(s), ${payload.policy_events.length} policy event(s) would be sent.`;
    }
    powerBiPreviewShown = true;
    const pushBtn = document.querySelector("[data-power-bi-push]");
    if (pushBtn) pushBtn.disabled = false;
    if (note) note.textContent = "";
  } catch (error) {
    if (note) note.textContent = `Preview failed: ${formatInvokeError(error)}`;
    else toastError(`Power BI preview failed: ${formatInvokeError(error)}`);
  }
}

async function pushPowerBiExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  if (!powerBiPreviewShown) return;
  const note = document.getElementById("power-bi-note");
  if (note) note.textContent = "Pushing to Power BI…";
  try {
    const result = await invoke("power_bi_push_audit_export", { sinceDays: null });
    if (note) {
      note.textContent = `Pushed ${result.runs_pushed} run(s), ${result.actions_pushed} action(s), ${result.policy_events_pushed} policy event(s).`;
    }
    showNotification("Pushed to Power BI.");
  } catch (error) {
    if (note) note.textContent = `Push failed: ${formatInvokeError(error)}`;
    else toastError(`Power BI push failed: ${formatInvokeError(error)}`);
  }
}

async function revokePowerBiGrant() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  try {
    await invoke("power_bi_revoke_grant");
    powerBiPreviewShown = false;
    showNotification("Power BI disconnected.");
    openSettings();
  } catch (error) {
    toastError(`Power BI disconnect failed: ${formatInvokeError(error)}`);
  }
}

// --- Microsoft Fabric (grant + workspace list + export preview) ---------------
async function connectFabric() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("fabric-note");
  if (note) note.textContent = "Opening your browser to connect Fabric…";
  try {
    await invoke("fabric_request_grant");
    showNotification("Fabric connected.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Connect failed: ${formatInvokeError(error)}`;
    else toastError(`Fabric connect failed: ${formatInvokeError(error)}`);
  }
}

async function listFabricWorkspaces() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("fabric-note");
  const select = document.getElementById("fabric-workspace-id");
  const lhSelect = document.getElementById("fabric-lakehouse-id");
  try {
    const workspaces = await invoke("fabric_list_workspaces");
    if (select) {
      const prev = select.value;
      select.innerHTML =
        '<option value="">Select workspace…</option>' +
        workspaces
          .map(
            (w) =>
              `<option value="${escapeAttr(w.id)}">${escapeHtml(w.display_name)}</option>`,
          )
          .join("");
      if (prev && workspaces.some((w) => w.id === prev)) {
        select.value = prev;
      } else if (workspaces[0]?.id) {
        select.value = workspaces[0].id;
      }
    }
    if (lhSelect) {
      lhSelect.innerHTML = '<option value="">Select lakehouse…</option>';
      lhSelect.disabled = !select?.value;
    }
    if (select?.value) {
      await listFabricLakehouses();
    }
    if (note) note.textContent = workspaces.length ? "" : "No workspaces returned.";
  } catch (error) {
    if (note) note.textContent = `Workspace list failed: ${formatInvokeError(error)}`;
    else toastError(`Fabric workspace list failed: ${formatInvokeError(error)}`);
  }
}

async function listFabricLakehouses() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const workspaceId = document.getElementById("fabric-workspace-id")?.value?.trim();
  const note = document.getElementById("fabric-note");
  const select = document.getElementById("fabric-lakehouse-id");
  if (!workspaceId) {
    if (select) {
      select.innerHTML = '<option value="">Select lakehouse…</option>';
      select.disabled = true;
    }
    return;
  }
  try {
    const lakehouses = await invoke("fabric_list_lakehouses", { workspaceId });
    if (select) {
      const prev = select.value;
      select.disabled = false;
      select.innerHTML =
        '<option value="">Select lakehouse…</option>' +
        lakehouses
          .map(
            (lh) =>
              `<option value="${escapeAttr(lh.id)}">${escapeHtml(lh.display_name)}</option>`,
          )
          .join("");
      if (prev && lakehouses.some((lh) => lh.id === prev)) {
        select.value = prev;
      } else if (lakehouses[0]?.id) {
        select.value = lakehouses[0].id;
      }
    }
    if (note) note.textContent = lakehouses.length ? "" : "No lakehouses in this workspace.";
  } catch (error) {
    if (note) note.textContent = `Lakehouse list failed: ${formatInvokeError(error)}`;
    else toastError(`Fabric lakehouse list failed: ${formatInvokeError(error)}`);
  }
}

async function previewFabricExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("fabric-note");
  const pre = document.getElementById("fabric-preview");
  try {
    const payload = await invoke("fabric_export_preview", { sinceDays: null });
    if (pre) {
      pre.style.display = "block";
      pre.textContent = `${payload.runs.length} run(s), ${payload.actions.length} action(s), ${payload.policy_events.length} policy event(s) would be exported.`;
    }
    fabricPreviewShown = true;
    const pushBtn = document.querySelector("[data-fabric-push]");
    if (pushBtn) pushBtn.disabled = false;
    if (note) note.textContent = "";
  } catch (error) {
    if (note) note.textContent = `Preview failed: ${formatInvokeError(error)}`;
    else toastError(`Fabric preview failed: ${formatInvokeError(error)}`);
  }
}

async function pushFabricExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  if (!fabricPreviewShown) return;
  const workspaceId = document.getElementById("fabric-workspace-id")?.value?.trim();
  const lakehouseId = document.getElementById("fabric-lakehouse-id")?.value?.trim();
  if (!workspaceId || !lakehouseId) {
    return toastError("Select a workspace and lakehouse first.");
  }
  const note = document.getElementById("fabric-note");
  if (note) note.textContent = "Pushing to Fabric lakehouse…";
  try {
    const result = await invoke("fabric_push_audit_export", {
      workspaceId,
      lakehouseId,
      sinceDays: null,
    });
    if (note) {
      note.textContent = `Pushed ${result.runs_pushed} run(s) to ${result.export_prefix}.`;
    }
    showNotification("Pushed to Fabric lakehouse.");
  } catch (error) {
    if (note) note.textContent = `Push failed: ${formatInvokeError(error)}`;
    else toastError(`Fabric push failed: ${formatInvokeError(error)}`);
  }
}

async function revokeFabricGrant() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  try {
    await invoke("fabric_revoke_grant");
    fabricPreviewShown = false;
    showNotification("Fabric disconnected.");
    openSettings();
  } catch (error) {
    toastError(`Fabric disconnect failed: ${formatInvokeError(error)}`);
  }
}

// --- Google Cloud Storage export ------------------------------------------------
async function connectGoogleCloud() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("google-note");
  if (note) note.textContent = "Opening your browser to connect Google Cloud…";
  try {
    await invoke("google_request_grant");
    showNotification("Google Cloud connected.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Connect failed: ${formatInvokeError(error)}`;
    else toastError(`Google Cloud connect failed: ${formatInvokeError(error)}`);
  }
}

async function listGoogleBuckets() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const projectId = document.getElementById("google-project-id")?.value?.trim();
  if (!projectId) return toastError("Enter your GCP project ID first.");
  const note = document.getElementById("google-note");
  const select = document.getElementById("google-bucket");
  try {
    const buckets = await invoke("google_list_buckets", { projectId });
    if (select) {
      select.innerHTML =
        '<option value="">Select bucket…</option>' +
        buckets
          .map((b) => `<option value="${escapeAttr(b.name)}">${escapeHtml(b.name)}</option>`)
          .join("");
    }
    if (note) note.textContent = buckets.length ? "" : "No buckets returned for this project.";
  } catch (error) {
    if (note) note.textContent = `Bucket list failed: ${formatInvokeError(error)}`;
    else toastError(`GCS bucket list failed: ${formatInvokeError(error)}`);
  }
}

async function previewGoogleExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("google-note");
  const pre = document.getElementById("google-preview");
  try {
    const payload = await invoke("google_export_preview", { sinceDays: null });
    if (pre) {
      pre.style.display = "block";
      pre.textContent = `${payload.runs.length} run(s), ${payload.actions.length} action(s), ${payload.policy_events.length} policy event(s) would be exported.`;
    }
    googlePreviewShown = true;
    const pushBtn = document.querySelector("[data-google-push]");
    if (pushBtn) pushBtn.disabled = false;
    if (note) note.textContent = "";
  } catch (error) {
    if (note) note.textContent = `Preview failed: ${formatInvokeError(error)}`;
    else toastError(`Google export preview failed: ${formatInvokeError(error)}`);
  }
}

async function pushGoogleExport() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  if (!googlePreviewShown) return;
  const bucket = document.getElementById("google-bucket")?.value?.trim();
  if (!bucket) return toastError("Select a bucket first.");
  const note = document.getElementById("google-note");
  if (note) note.textContent = "Pushing to GCS…";
  try {
    const result = await invoke("google_push_audit_export", { bucket, sinceDays: null });
    if (note) {
      note.textContent = `Pushed ${result.runs_pushed} run(s) to gs://${escapeAttr(bucket)}/${escapeAttr(result.export_prefix)}.`;
    }
    showNotification("Pushed to Google Cloud Storage.");
  } catch (error) {
    if (note) note.textContent = `Push failed: ${formatInvokeError(error)}`;
    else toastError(`GCS push failed: ${formatInvokeError(error)}`);
  }
}

async function revokeGoogleCloudGrant() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  try {
    await invoke("google_revoke_grant");
    googlePreviewShown = false;
    showNotification("Google Cloud disconnected.");
    openSettings();
  } catch (error) {
    toastError(`Google Cloud disconnect failed: ${formatInvokeError(error)}`);
  }
}

async function enableMcpPairing() {
  if (!invoke) return notAvailable();
  const note = document.getElementById("mcp-pairing-note");
  const pre = document.getElementById("mcp-pairing-code");
  try {
    const result = await invoke("mcp_enable_pairing");
    if (pre) {
      pre.style.display = "block";
      pre.textContent = `Pairing code (copy into your MCP client initialize params):\n${result.code}`;
    }
    if (note) note.textContent = "Pairing enabled. Clients must send this code on initialize.";
    showNotification("MCP pairing enabled.");
  } catch (error) {
    toastError(`Could not enable MCP pairing: ${formatInvokeError(error)}`);
  }
}

async function bindGoogleExportBucket() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const bucket = document.getElementById("google-bucket")?.value?.trim();
  if (!bucket) return toastError("Select a bucket first.");
  const note = document.getElementById("google-note");
  try {
    await invoke("google_bind_export_bucket", { bucket });
    if (note) note.textContent = `Export grant bound to gs://${escapeAttr(bucket)}.`;
    showNotification("GCS export bucket bound.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Bind failed: ${formatInvokeError(error)}`;
    else toastError(`GCS bind failed: ${formatInvokeError(error)}`);
  }
}

async function generateFabricWebhookSecret() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const pre = document.getElementById("fabric-webhook-secret");
  try {
    const secret = await invoke("fabric_set_webhook_secret");
    if (pre) {
      pre.style.display = "block";
      pre.textContent = `Webhook secret (copy into your Fabric/Eventstream connector):\n${secret}`;
    }
    showNotification("Fabric webhook secret generated.");
    openSettings();
  } catch (error) {
    toastError(`Webhook secret failed: ${formatInvokeError(error)}`);
  }
}

async function startMcpHttpServer() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const port = Number(document.getElementById("mcp-http-port")?.value) || 8787;
  const bearerToken = document.getElementById("mcp-http-bearer")?.value?.trim() || null;
  const tlsCertPath = document.getElementById("mcp-http-tls-cert")?.value?.trim() || null;
  const tlsKeyPath = document.getElementById("mcp-http-tls-key")?.value?.trim() || null;
  const exposeLan = document.getElementById("mcp-http-lan")?.checked || false;
  const note = document.getElementById("mcp-pairing-note");
  try {
    const status = await invoke("mcp_start_http_server", {
      port,
      exposeLan,
      bearerToken,
      tlsCertPath,
      tlsKeyPath,
    });
    if (note) {
      note.textContent = `HTTP server running on ${status.bind_host}:${status.port}. POST /mcp for MCP; POST /fabric/webhook for inbound intents.`;
    }
    showNotification("MCP HTTP server started.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Start failed: ${formatInvokeError(error)}`;
    else toastError(`MCP HTTP start failed: ${formatInvokeError(error)}`);
  }
}

async function stopMcpHttpServer() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("mcp-pairing-note");
  try {
    await invoke("mcp_stop_http_server");
    if (note) note.textContent = "HTTP server stopped.";
    showNotification("MCP HTTP server stopped.");
    openSettings();
  } catch (error) {
    toastError(`MCP HTTP stop failed: ${formatInvokeError(error)}`);
  }
}

async function startMcpRelay() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const relayUrl = document.getElementById("mcp-relay-url")?.value?.trim();
  const deviceId = document.getElementById("mcp-relay-device-id")?.value?.trim();
  const deviceToken = document.getElementById("mcp-relay-token")?.value?.trim();
  if (!relayUrl || !deviceId || !deviceToken) {
    return toastError("Relay URL, device ID, and token are required.");
  }
  const note = document.getElementById("mcp-pairing-note");
  try {
    const status = await invoke("mcp_start_relay", {
      relayUrl,
      deviceId,
      deviceToken,
    });
    if (note) {
      note.textContent = `Cloud relay connected to ${status.relay_url} as ${status.device_id}.`;
    }
    showNotification("MCP cloud relay connected.");
    openSettings();
  } catch (error) {
    if (note) note.textContent = `Relay connect failed: ${formatInvokeError(error)}`;
    else toastError(`MCP relay failed: ${formatInvokeError(error)}`);
  }
}

async function stopMcpRelay() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return;
  const note = document.getElementById("mcp-pairing-note");
  try {
    await invoke("mcp_stop_relay");
    if (note) note.textContent = "Cloud relay disconnected.";
    showNotification("MCP cloud relay disconnected.");
    openSettings();
  } catch (error) {
    toastError(`MCP relay stop failed: ${formatInvokeError(error)}`);
  }
}

async function disableMcpPairing() {
  if (!invoke) return notAvailable();
  try {
    await invoke("mcp_disable_pairing");
    showNotification("MCP pairing disabled.");
    openSettings();
  } catch (error) {
    toastError(`Could not disable MCP pairing: ${formatInvokeError(error)}`);
  }
}

async function signInWithProvider(provider) {
  if (!invoke) return notAvailable();
  const note = document.getElementById("account-status-note");
  const btn = document.querySelector(`[data-account-sign-in="${provider}"]`);
  if (btn?.disabled) {
    const msg =
      provider === "microsoft"
        ? "Microsoft sign-in is not configured yet. Add integrations.microsoft_client_id in config or set GHOST_MS_CLIENT_ID for local dev."
        : "Google sign-in is not configured.";
    if (note) note.textContent = msg;
    else toastError(msg);
    return;
  }
  if (btn) btn.disabled = true;
  if (note) note.textContent = "Opening your browser to sign in…";
  try {
    await invoke("account_sign_in", { provider });
    showNotification("Signed in.");
    openSettings();
  } catch (error) {
    console.error("Sign-in failed:", error);
    if (note) note.textContent = `Sign-in failed: ${formatInvokeError(error)}`;
    else toastError(`Sign-in failed: ${formatInvokeError(error)}`);
  } finally {
    if (btn) btn.disabled = false;
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
      if (data.chars && data.chars.trim()) return `Typed ${prefix}[redacted]`;
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
        statusEl.style.color = "#0e8f78";
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
  const reviewBtn = document.getElementById("reviewBtn");
  if (reviewBtn) reviewBtn.disabled = isRecording || recordedEvents.length === 0;
  if (cancelBtn) cancelBtn.disabled = !isPlaying;
  if (pauseBtn) pauseBtn.disabled = !isPlaying || isPaused;
  if (resumeBtn) resumeBtn.disabled = !isPlaying || !isPaused;

  // Contextual control groups: replay actions appear once a workflow exists;
  // live playback controls appear only while a replay is running.
  const replayControls = document.getElementById("replayControls");
  if (replayControls) replayControls.hidden = recordedEvents.length === 0;
  const playbackControls = document.getElementById("playbackControls");
  if (playbackControls) playbackControls.hidden = !isPlaying;

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
    toastError("Failed to start observer: " + formatInvokeError(error));
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
    toastError("Failed to observe: " + formatInvokeError(error));
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
    toastError("Failed to generate insights: " + formatInvokeError(error));
  }
}

function displaySuggestions(suggestions) {
  const modal = document.getElementById("analysis-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>${icon("i-robot")} Proactive Automation Suggestions</h3>
    ${suggestions.map((s, i) => `
      <div style="margin: 12px 0; padding: 12px; background: rgba(14, 143, 120, 0.1); border-radius: 8px; border-left: 3px solid #0e8f78;">
        <p><strong>${i + 1}. ${escapeHtml(s.suggestion)}</strong></p>
        <p style="font-size: 0.9rem; color: #9ca3af;">Suggested workflow: <code>${escapeHtml(s.suggested_workflow_name)}</code></p>
        <p style="font-size: 0.85rem;">Confidence: ${(s.confidence * 100).toFixed(1)}%</p>
        <button data-create-workflow-from-suggestion data-workflow-name="${escapeAttr(s.suggested_workflow_name)}" data-pattern-id="${escapeAttr(s.pattern_id)}" style="margin-top: 8px; font-size: 0.85rem;">Create This Workflow</button>
      </div>
    `).join("")}
    <button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button>
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
    toastError("Failed to save workflow: " + formatInvokeError(error));
  }
}

function displayGeekInsights(insights, appName) {
  const modal = document.getElementById("analysis-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>${icon("i-wrench")} Geek Mode: Technical Insights for ${escapeHtml(appName)}</h3>
    <div style="margin: 12px 0;">
      <h4 style="color: #0e8f78;">Performance Metrics</h4>
      <p>Total Duration: ${insights.performance_metrics.total_duration_ms}ms</p>
      <p>Avg Delay: ${insights.performance_metrics.avg_delay_ms.toFixed(2)}ms</p>
      ${insights.performance_metrics.bottleneck_events.length > 0 ? `
        <p>Bottleneck Events: ${escapeHtml(insights.performance_metrics.bottleneck_events.join(", "))}</p>
      ` : ""}
    </div>
    <div style="margin: 12px 0;">
      <h4 style="color: #0e8f78;">Event Timing Analysis</h4>
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
    <button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button>
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
    toastError("Replay failed: " + formatInvokeError(error));
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
    toastError("Capture failed: " + formatInvokeError(error));
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
  void refreshTimeline();
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
    toastError("Create failed: " + formatInvokeError(error));
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
    toastError("Load failed: " + formatInvokeError(error));
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
  await organizerPrefillPaths();
  if (!invoke) return; // static mode: the panel stays inert
  await organizerRefreshZones();
  await organizerCheckUnfinishedRun();
  await organizerCheckFabricInbound();
}

async function organizerCheckFabricInbound() {
  const banner = document.getElementById("organizerFabricInboundBanner");
  if (!banner || !invoke || !experimentalEnabled) {
    if (banner) banner.innerHTML = "";
    return;
  }
  let intents;
  try {
    intents = await invoke("fabric_list_inbound_intents");
  } catch (_err) {
    return;
  }
  const pending = (intents || []).filter((i) => i.status === "pending");
  if (!pending.length) {
    banner.innerHTML = "";
    return;
  }
  const first = pending[0];
  banner.innerHTML = `
    <section class="banner banner--warn">
      <span><strong>Fabric inbound intent:</strong> ${escapeHtml(first.summary)} — review and scan before approving. Ghost will not auto-execute.</span>
      <div class="banner__actions">
        <button class="btn btn--ghost btn--small" id="organizerFabricInboundReviewBtn" type="button">Review Zone</button>
        <button class="btn btn--ghost btn--small" id="organizerFabricInboundDismissBtn" type="button">Dismiss</button>
      </div>
    </section>`;
  document.getElementById("organizerFabricInboundReviewBtn")?.addEventListener("click", async () => {
    if (first.zone_id) {
      organizerSelectedZoneId = first.zone_id;
      await organizerRefreshZones();
    }
    showNotification("Scan and review the plan — inbound Fabric signals never auto-run.");
  });
  document.getElementById("organizerFabricInboundDismissBtn")?.addEventListener("click", async () => {
    try {
      await invoke("fabric_dismiss_inbound_intent", { intentId: first.intent_id });
      showNotification("Inbound Fabric intent dismissed.");
    } catch (err) {
      toastError("Could not dismiss: " + formatInvokeError(err));
    }
    await organizerCheckFabricInbound();
  });
}

// A run that began (files may already have moved) but never finished —
// almost always because the app crashed or was killed mid-run. Ghost wrote
// undo data for every step it completed before that happened
// (write-ahead durability — see docs/organizer-executor.md), so nothing is
// lost; the user just needs to say what to do about it: undo what was
// applied, or leave it as-is.
async function organizerCheckUnfinishedRun() {
  const banner = document.getElementById("organizerUnfinishedBanner");
  if (!banner || !invoke) return;
  let found;
  try {
    found = await invoke("organizer_check_unfinished_run");
  } catch (_err) {
    return; // best-effort; a failed check must not block the Organizer view
  }
  if (!found) {
    banner.innerHTML = "";
    return;
  }
  banner.innerHTML = `
    <section class="banner banner--warn">
      <span><strong>Ghost was interrupted mid-run.</strong> Before it stopped, it applied
      ${found.applied} change(s) to your files. You can undo those changes now, or leave them as they are.</span>
      <div class="banner__actions">
        <button class="btn btn--ghost btn--small" id="organizerUnfinishedUndoBtn" type="button">Undo those changes</button>
        <button class="btn btn--ghost btn--small" id="organizerUnfinishedDismissBtn" type="button">Leave as-is</button>
      </div>
    </section>`;
  document.getElementById("organizerUnfinishedUndoBtn")?.addEventListener("click", async () => {
    await organizerUndo(found.id);
    await organizerCheckUnfinishedRun();
  });
  document.getElementById("organizerUnfinishedDismissBtn")?.addEventListener("click", async () => {
    try {
      await invoke("organizer_dismiss_unfinished_run", { executionId: found.id });
      showNotification("Left the interrupted run's changes as-is.", "info");
    } catch (err) {
      toastError("Could not dismiss: " + formatInvokeError(err));
    }
    await organizerCheckUnfinishedRun();
  });
}

function organizerSelectedZone() {
  return organizerZones.find((z) => z.id === organizerSelectedZoneId) || null;
}

async function organizerRefreshZones() {
  try {
    organizerZones = await invoke("organizer_list_zones");
  } catch (err) {
    organizerZones = [];
    toastError("Could not load Zones: " + formatInvokeError(err));
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
    toastError("Could not load folders: " + formatInvokeError(err));
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
        const trust = r.trust || "ask_first";
        // A per-rule trust selector: the boundary is legible and adjustable in
        // place. The backend re-checks policy on every action, so this only
        // records intent — it can never widen what Ghost will actually do.
        const trustSelect = `<select class="org-trust-select" data-rule-path="${escapeAttr(r.path)}" title="How much autonomy this folder carries">
            <option value="automate"${trust === "automate" ? " selected" : ""}>Automate</option>
            <option value="ask_first"${trust === "ask_first" ? " selected" : ""}>Ask first</option>
            <option value="never"${trust === "never" ? " selected" : ""}>Never</option>
          </select>`;
        return `<span class="organizer-rule-chip">${escapeHtml(r.path)} <em>(${escapeHtml(grants || "no permissions")})</em> ${organizerTrustBadge(trust)}${trustSelect}</span>`;
      })
      .join("");
    // Wire each in-place trust change to the backend.
    list.querySelectorAll(".org-trust-select").forEach((sel) =>
      sel.addEventListener("change", () =>
        organizerSetRuleTrust(sel.dataset.rulePath, sel.value),
      ),
    );
  }
  organizerUpdateButtons(rules);
}

// The user-facing trust vocabulary, mirrored from the policy engine's
// TrustLevel: automate runs without asking, ask-first needs approval, never
// refuses. Shown wherever a rule or a planned action is displayed.
function organizerTrustBadge(trust) {
  switch (trust) {
    case "automate":
      return `<span class="org-badge org-badge--automate" title="Runs without asking each time">Automate</span>`;
    case "never":
      return `<span class="org-badge org-badge--never" title="Refuses changes in this folder">Never</span>`;
    case "ask_first":
    default:
      return `<span class="org-badge org-badge--confirm" title="Asks for approval per action">Ask first</span>`;
  }
}

async function organizerSetRuleTrust(path, trust) {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone || !path) return;
  try {
    await invoke("organizer_set_rule_trust", { zoneId: zone.id, path, trust });
    // Trust changes what the next plan proposes; force a fresh review.
    organizerHasReviewedPlan = false;
    await organizerRefreshRules();
    showNotification(`Trust for ${path} set to ${trust.replace("_", " ")}.`, "info");
  } catch (err) {
    toastError("Could not update trust: " + formatInvokeError(err));
    await organizerRefreshRules();
  }
}

function organizerUpdateButtons(rules) {
  const scanBtn = document.getElementById("organizerScanBtn");
  const runBtn = document.getElementById("organizerRunBtn");
  const aiBtn = document.getElementById("organizerAiSuggestBtn");
  const mcpBtn = document.getElementById("organizerMcpTokenBtn");
  const hasFolders = Array.isArray(rules) && rules.length > 0;
  if (scanBtn) scanBtn.disabled = !organizerSelectedZone() || !hasFolders;
  if (runBtn) runBtn.disabled = !organizerHasReviewedPlan;
  if (aiBtn) aiBtn.disabled = !organizerSelectedZone() || !hasFolders;
  if (mcpBtn) mcpBtn.disabled = !organizerHasReviewedPlan;
}

async function organizerCreateZone() {
  if (!invoke) return notAvailable();
  const name = await ghostPrompt("Name this Zone (e.g. Downloads cleanup)", "", "Zone name");
  if (!name) return;
  try {
    const zone = await invoke("organizer_create_zone", { name, description: null, renameDated: false });
    organizerSelectedZoneId = zone.id;
    await organizerRefreshZones();
    showNotification(`Zone "${zone.name}" created. Add the folder to organize.`, "info");
  } catch (err) {
    toastError("Could not create Zone: " + formatInvokeError(err));
  }
}

// Export the selected Zone's boundary and folder rules as a portable JSON
// policy pack — the write side of organizer_import_policy_pack, for sharing
// a reviewed boundary definition across a team's devices without cloud sync.
async function organizerExportPolicyPack() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Select a Zone first.");
  try {
    const json = await invoke("organizer_export_policy_pack", { zoneId: zone.id });
    const blob = new Blob([json], { type: "application/json;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    const slug = zone.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "zone";
    a.href = url;
    a.download = `ghost-policy-pack-${slug}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showNotification(`Policy pack exported for "${zone.name}".`, "info");
  } catch (err) {
    toastError("Export failed: " + formatInvokeError(err));
  }
}

function organizerImportPolicyPack() {
  document.getElementById("organizerImportPackInput")?.click();
}

async function organizerHandlePolicyPackFile(file) {
  if (!invoke || !file) return;
  try {
    const text = await file.text();
    const zone = await invoke("organizer_import_policy_pack", { packJson: text });
    organizerSelectedZoneId = zone.id;
    await organizerRefreshZones();
    showNotification(`Imported policy pack as Zone "${zone.name}". Review its rules before running it.`, "info");
  } catch (err) {
    toastError("Import failed: " + formatInvokeError(err));
  }
}

// The guided setup presets. Each turns a short "interview" (a few questions)
// into a named Zone and its folder rules — Ghost's local, permission-bounded
// answer to the "configure yourself from how the team works" onboarding, with
// no cloud and no credentials. `mode` is "two_folder" (move OUT of a source
// INTO a destination root) or "in_place" (organize one folder where it sits).
const ORGANIZER_PRESETS = {
  "Client filing": {
    description: "File invoices, receipts, and statements into dated client folders.",
    mode: "two_folder",
    renameDated: true,
    sourcePrompt: "Full path of the folder client documents arrive in",
    sourceKey: "downloads",
    destPrompt: "Destination root for filed client documents",
    destKey: "documents",
  },
  "Bookkeeping inbox": {
    description: "Sort a bookkeeping inbox into category folders, dated for filing.",
    mode: "in_place",
    renameDated: true,
    sourcePrompt: "Full path of the bookkeeping inbox to organize",
    sourceKey: "documents",
  },
  "Downloads cleanup": {
    description: "Tidy a Downloads-style folder into category folders in place.",
    mode: "in_place",
    renameDated: false,
    sourcePrompt: "Full path of the folder to tidy",
    sourceKey: "downloads",
  },
};

function organizerPresetPath(key) {
  if (key === "downloads") return organizerSuggestedDownloads();
  if (key === "documents") return organizerSuggestedDocuments();
  return organizerPathDefaults.home || "";
}

const TRUST_CHOICES = {
  "Ask first (recommended) — approve each change": "ask_first",
  "Automate — run without asking each time": "automate",
  "Never — set up the boundary but make no changes": "never",
};

// Guided wizard: pick a preset, answer a couple of questions, choose a trust
// level, confirm the exact rules, then create the Zone and preview the plan.
// Reuses the in-app dialog helpers; no new backend — it composes the existing
// organizer_create_zone / organizer_add_folder_rule commands.
async function organizerRunWizard() {
  if (!invoke) return notAvailable();

  const presetName = await ghostPick(
    "What do you want to set up? (Ghost stays local — no cloud, no logins.)",
    Object.keys(ORGANIZER_PRESETS),
  );
  if (!presetName) return;
  const preset = ORGANIZER_PRESETS[presetName];

  // Prefill with real local folders. Backend also expands ~/… if needed.
  const sourceDefault = organizerPresetPath(preset.sourceKey);
  const sourcePath = (
    await ghostPrompt(preset.sourcePrompt, sourceDefault, sourceDefault)
  )?.trim();
  if (!sourcePath) return;

  let destPath = sourcePath;
  if (preset.mode === "two_folder") {
    const destDefault = organizerPresetPath(preset.destKey);
    destPath = (await ghostPrompt(preset.destPrompt, destDefault, destDefault))?.trim();
    if (!destPath) return;
  }

  const trustLabel = await ghostPick(
    "How much autonomy should this Zone have? You can change it per folder later.",
    Object.keys(TRUST_CHOICES),
  );
  if (!trustLabel) return;
  const trust = TRUST_CHOICES[trustLabel];

  // Build the rules this preset needs. In two-folder mode the source grants
  // read + move-OUT (a read-only source makes the policy engine deny every
  // filing move) and the destination grants read/create/rename/move. In-place
  // mode is a single full-permission rule. The chosen trust rides on the
  // mutating rules; the engine still re-checks every action at execution.
  const rules = [];
  if (preset.mode === "two_folder" && destPath !== sourcePath) {
    rules.push({
      path: sourcePath,
      can_read: true,
      can_create: false,
      can_rename: false,
      can_move: true,
      can_copy: false,
      can_delete: false,
      trust,
    });
    rules.push({
      path: destPath,
      can_read: true,
      can_create: true,
      can_rename: true,
      can_move: true,
      can_copy: false,
      can_delete: false,
      trust,
    });
  } else {
    rules.push({
      path: sourcePath,
      can_read: true,
      can_create: true,
      can_rename: true,
      can_move: true,
      can_copy: false,
      can_delete: false,
      trust,
    });
  }

  // Final review: show exactly the boundary the wizard will create before it
  // writes anything, so the user approves the playbook, not a black box.
  const preview = rules
    .map((r) => `• ${r.path} — ${organizerGrantSummary(r)} · trust: ${trust.replace("_", " ")}`)
    .join("\n");
  const confirmed = await ghostConfirm(
    `Create the "${presetName}" Zone with these folders?\n\n${preview}\n\nGhost will preview the plan next — nothing moves until you approve it.`,
    "Create Zone",
  );
  if (!confirmed) return;

  try {
    const zone = await invoke("organizer_create_zone", {
      name: presetName,
      description: preset.description,
      renameDated: preset.renameDated,
    });
    organizerSelectedZoneId = zone.id;
    for (const rule of rules) {
      await invoke("organizer_add_folder_rule", { zoneId: zone.id, rule });
    }
    organizerHasReviewedPlan = false;
    await organizerRefreshZones();
    showNotification(`"${presetName}" Zone created. Previewing the plan now.`, "info");
    await organizerScan();
  } catch (err) {
    toastError("Could not create the Zone: " + formatInvokeError(err));
  }
}

// Compact grant summary for the wizard's confirmation preview.
function organizerGrantSummary(rule) {
  const grants = [
    rule.can_read && "read",
    rule.can_create && "create",
    rule.can_move && "move",
    rule.can_rename && "rename",
  ]
    .filter(Boolean)
    .join(", ");
  return grants || "no permissions";
}


/** Real Downloads/home paths for placeholders + guided setup (no /Users/you). */
let organizerPathDefaults = { downloads: null, home: null, documents: null };

async function organizerPrefillPaths() {
  try {
    organizerPathDefaults = (await invoke("organizer_default_paths")) || organizerPathDefaults;
  } catch (err) {
    console.warn("organizer_default_paths unavailable:", err);
  }
  const input = document.getElementById("organizerFolderPath");
  if (!input) return;
  const suggested = organizerPathDefaults.downloads || organizerPathDefaults.home || "";
  if (suggested) {
    input.placeholder = suggested;
    if (!input.value.trim()) input.value = suggested;
  }
}

function organizerSuggestedDownloads() {
  return organizerPathDefaults.downloads || "";
}

function organizerSuggestedDocuments() {
  return organizerPathDefaults.documents || organizerPathDefaults.home || "";
}

/** Native folder picker — avoids fake `/Users/you/…` paths on first run. */
async function organizerBrowseFolder() {
  const dialogApi = window.__TAURI__?.dialog;
  if (!dialogApi?.open) {
    showNotification("Type a full folder path, or use Browse after a desktop rebuild.", "info");
    return;
  }
  try {
    const selected = await dialogApi.open({
      directory: true,
      multiple: false,
      title: "Choose a folder Ghost may organize",
      defaultPath: organizerSuggestedDownloads() || undefined,
    });
    if (selected == null || selected === false) return;
    const path = Array.isArray(selected) ? selected[0] : selected;
    const input = document.getElementById("organizerFolderPath");
    if (input && path) {
      input.value = path;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
  } catch (err) {
    toastError("Could not open folder picker: " + formatInvokeError(err));
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
    trust: document.getElementById("permTrust")?.value || "ask_first",
  };
  try {
    await invoke("organizer_add_folder_rule", { zoneId: zone.id, rule });
    const input = document.getElementById("organizerFolderPath");
    if (input) input.value = "";
    organizerHasReviewedPlan = false;
    await organizerRefreshRules();
    showNotification("Folder added to the Zone.", "info");
  } catch (err) {
    toastError("Could not add folder: " + formatInvokeError(err));
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

// How an executed action was authorized: automatically (an "automate" rule) or
// under the user's explicit approval of the run.
function organizerProvenanceBadge(provenance) {
  if (provenance === "automated")
    return ` <span class="org-badge org-badge--automate" title="Ran without asking (automate rule)">automated</span>`;
  if (provenance === "user_approved")
    return ` <span class="org-badge org-badge--confirm" title="Ran because you approved this run">you approved</span>`;
  return "";
}

// Export a run's audit log as CSV or JSON. The backend returns the text; we
// hand it to the user as a download so they keep a portable record of exactly
// what Ghost did and under which rule.
const ORGANIZER_EXPORT_FORMATS = {
  csv: { mime: "text/csv", ext: "csv", label: "CSV" },
  json: { mime: "application/json", ext: "json", label: "JSON" },
  compliance: { mime: "text/plain", ext: "txt", label: "a compliance report" },
  // A compliance report plus a cryptographic signature over its own content,
  // verifiable offline against this machine's signing key via
  // organizer_verify_signed_report — see organizerVerifySignedReport().
  signed: { mime: "text/plain", ext: "txt", label: "a signed report" },
};

async function organizerExportAudit(executionId, format) {
  if (!invoke) return notAvailable();
  if (!executionId) return;
  const spec = ORGANIZER_EXPORT_FORMATS[format] || { mime: "text/plain", ext: format, label: format.toUpperCase() };
  try {
    const text = await invoke("organizer_export_audit", { executionId, format });
    const blob = new Blob([text], { type: `${spec.mime};charset=utf-8` });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `ghost-audit-${executionId}.${spec.ext}`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showNotification(`Audit exported as ${spec.label}.`, "info");
  } catch (err) {
    toastError("Export failed: " + formatInvokeError(err));
  }
}

function organizerPlanHasApplyableActions(plan) {
  return (plan?.actions || []).some(
    (a) =>
      a.decision?.decision === "allow" || a.decision?.decision === "require_confirmation",
  );
}

async function organizerScan() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Create a Zone first.");
  const scanBtn = document.getElementById("organizerScanBtn");
  const prevLabel = scanBtn?.textContent;
  if (scanBtn) {
    scanBtn.disabled = true;
    scanBtn.textContent = "Scanning…";
  }
  const result = document.getElementById("organizerResult");
  if (result) result.innerHTML = `<p class="organizer-muted">Scanning… nothing has been changed.</p>`;

  let plan;
  try {
    plan = await invoke("organizer_plan", { zoneId: zone.id });
  } catch (err) {
    if (result) result.innerHTML = "";
    toastError("Scan failed: " + formatInvokeError(err));
    if (scanBtn) {
      scanBtn.disabled = false;
      scanBtn.textContent = prevLabel || "Scan folder";
    }
    return;
  } finally {
    if (scanBtn) scanBtn.textContent = prevLabel || "Scan folder";
  }
  organizerRenderPlan(plan);
  organizerHasReviewedPlan = organizerPlanHasApplyableActions(plan);
  organizerUpdateButtons(await safeRules(zone.id));
  if (!organizerHasReviewedPlan) {
    showInsight("Nothing to apply in this plan — add folder rules or pick a folder with matching files.");
  } else {
    showInsight("Preview ready. Nothing moved yet — review each step, then approve.");
  }
}

function organizerRenderAiSuggestion(result) {
  const resultEl = document.getElementById("organizerResult");
  if (!resultEl) return;
  const s = result.suggestion || {};
  const classifications = (s.classifications || [])
    .map(
      (c) =>
        `<li><code>${escapeHtml(c.zone_relative_path)}</code> → <strong>${escapeHtml(c.category)}</strong>${typeof c.confidence === "number" ? ` (${Math.round(c.confidence * 100)}%)` : ""}</li>`,
    )
    .join("");
  const rules = (s.proposed_rules || [])
    .map((r) => `<li>${escapeHtml(r.description)}${r.pattern_hint ? ` <span class="organizer-muted">(${escapeHtml(r.pattern_hint)})</span>` : ""}</li>`)
    .join("");
  const uncertainty = (s.uncertainty || [])
    .map((u) => `<li><code>${escapeHtml(u.zone_relative_path)}</code>: ${escapeHtml(u.reason)}</li>`)
    .join("");
  const assumptions = (s.assumptions || []).map((a) => `<li>${escapeHtml(a)}</li>`).join("");

  resultEl.innerHTML = `
    <div class="organizer-summary">
      <strong>AI suggestions</strong> · ${result.files_considered ?? 0} file(s) considered
      <span class="organizer-muted"> — suggestion only, nothing moved</span>
    </div>
    <p class="organizer-muted">${escapeHtml(s.objective || "")}</p>
    ${classifications ? `<h4 class="organizer-muted">Suggested classifications</h4><ul>${classifications}</ul>` : ""}
    ${rules ? `<h4 class="organizer-muted">Suggested rules</h4><ul>${rules}</ul>` : ""}
    ${assumptions ? `<h4 class="organizer-muted">Assumptions</h4><ul>${assumptions}</ul>` : ""}
    ${uncertainty ? `<h4 class="organizer-muted">Uncertain items</h4><ul>${uncertainty}</ul>` : ""}
    <p class="organizer-muted">Run <strong>Scan &amp; Preview</strong> to see Ghost's deterministic plan from your Zone rules. AI output never executes directly.</p>
  `;
}

async function organizerAiSuggest() {
  if (!invoke) return notAvailable();
  if (!experimentalEnabled) return toastError("AI suggestions require an experimental build.");
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Create a Zone first.");
  const btn = document.getElementById("organizerAiSuggestBtn");
  const prevLabel = btn?.textContent;
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Thinking…";
  }
  const result = document.getElementById("organizerResult");
  if (result) result.innerHTML = `<p class="organizer-muted">Requesting suggestions… nothing has been changed.</p>`;
  try {
    const suggestion = await invoke("organizer_intelligence_suggest", { zoneId: zone.id });
    organizerRenderAiSuggestion(suggestion);
    organizerHasReviewedPlan = false;
    organizerUpdateButtons(await safeRules(zone.id));
    showInsight("AI suggestions are advisory only — scan for the deterministic plan before approving.");
  } catch (err) {
    if (result) result.innerHTML = "";
    toastError("AI suggestions failed: " + formatInvokeError(err));
  } finally {
    if (btn) {
      btn.textContent = prevLabel || "AI suggestions";
      organizerUpdateButtons(await safeRules(zone.id));
    }
  }
}

async function organizerIssueMcpToken() {
  if (!invoke) return notAvailable();
  const zone = organizerSelectedZone();
  if (!zone) return toastError("Create a Zone first.");
  if (!organizerHasReviewedPlan) {
    return toastError("Scan and review a plan first — the token binds to the current server-side plan.");
  }
  try {
    const token = await invoke("organizer_issue_mcp_approval_token", { zoneId: zone.id });
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(token);
      showNotification("MCP approval token copied (valid ~5 minutes, single use).");
    } else {
      showInsight("MCP approval token issued — copy from the console.");
      console.info("MCP approval token:", token);
    }
  } catch (err) {
    toastError("Could not issue MCP token: " + formatInvokeError(err));
  }
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
      // Name the boundary that authorized (or refused) this action, so every
      // row answers "which rule fired?" the way an auditor would ask.
      const rule = a.rule_path
        ? `<div class="org-rule-attr">by rule <code>${escapeHtml(a.rule_path)}</code></div>`
        : "";
      return `<tr>
        <td>${organizerDescribeCapability(a.capability)} ${conflict} ${low}${rule}</td>
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

  const runBtn = document.getElementById("organizerRunBtn");
  const prevLabel = runBtn?.textContent;
  if (runBtn) {
    runBtn.disabled = true;
    runBtn.textContent = "Organizing…";
  }

  let res;
  try {
    res = await invoke("organizer_execute", { zoneId: zone.id });
  } catch (err) {
    toastError("Organize failed: " + formatInvokeError(err));
    if (runBtn) {
      runBtn.disabled = false;
      runBtn.textContent = prevLabel || "Approve & Organize";
    }
    return;
  }
  const r = res.report || {};
  const auditRows = (r.audit || [])
    .map((e) => {
      const outcome = e.outcome?.outcome || "?";
      const detail = e.outcome?.reason || e.outcome?.error || "";
      // Show the rule that fired and how the action was authorized — the same
      // "rule that fired / who signed off" record an auditor would expect.
      const rule = e.rule_path
        ? ` <span class="org-rule-attr">by rule <code>${escapeHtml(e.rule_path)}</code></span>`
        : "";
      const provenance = organizerProvenanceBadge(e.provenance);
      return `<li><span class="org-outcome org-outcome--${escapeAttr(outcome)}">${escapeHtml(outcome)}</span> ${organizerDescribeCapability(e.capability || {})}${provenance}${rule}${detail ? ` — <em>${escapeHtml(detail)}</em>` : ""}</li>`;
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
      <div class="btn-row">
        <button class="btn btn--ghost btn--small" id="organizerUndoLastBtn" type="button" data-exec-id="${escapeAttr(res.execution_id)}">Undo this run</button>
        <button class="btn btn--ghost btn--small" id="organizerExportCsvBtn" type="button">Export audit (CSV)</button>
        <button class="btn btn--ghost btn--small" id="organizerExportJsonBtn" type="button">Export audit (JSON)</button>
        <button class="btn btn--ghost btn--small" id="organizerExportSignedBtn" type="button" title="A compliance report with a cryptographic signature, verifiable offline">Export signed report</button>
      </div>
      <h3 class="organizer-subhead">Audit log</h3>
      <ul class="organizer-audit">${auditRows || "<li>No actions recorded.</li>"}</ul>`;
    const undoBtn = document.getElementById("organizerUndoLastBtn");
    if (undoBtn) undoBtn.addEventListener("click", () => organizerUndo(res.execution_id));
    const csvBtn = document.getElementById("organizerExportCsvBtn");
    if (csvBtn) csvBtn.addEventListener("click", () => organizerExportAudit(res.execution_id, "csv"));
    const jsonBtn = document.getElementById("organizerExportJsonBtn");
    if (jsonBtn) jsonBtn.addEventListener("click", () => organizerExportAudit(res.execution_id, "json"));
    const signedBtn = document.getElementById("organizerExportSignedBtn");
    if (signedBtn) signedBtn.addEventListener("click", () => organizerExportAudit(res.execution_id, "signed"));
  }
  organizerHasReviewedPlan = false;
  organizerUpdateButtons(await safeRules(zone.id));
  if (runBtn) runBtn.textContent = prevLabel || "Approve & Organize";
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
    toastError("Undo failed: " + formatInvokeError(err));
  }
}

// Labels for the local, first-touch milestones behind organizer_time_to_value —
// no paths or content, just when each stage of the trust pipeline first ran.
const ORGANIZER_MILESTONE_LABELS = {
  first_zone_created: "First Zone created",
  first_plan: "First plan previewed",
  first_run: "First run approved",
  first_undo: "First undo used",
};

function organizerRenderMilestones(milestones) {
  if (!milestones || milestones.length === 0) {
    return `<p class="organizer-muted">No milestones recorded yet — create a Zone to get started.</p>`;
  }
  const byKey = Object.fromEntries(milestones.map((m) => [m.key, m.at]));
  const items = ["first_zone_created", "first_plan", "first_run", "first_undo"]
    .filter((key) => byKey[key])
    .map((key) => {
      const when = new Date(Number(byKey[key]) * 1000).toLocaleString();
      return `<li><strong>${escapeHtml(ORGANIZER_MILESTONE_LABELS[key])}:</strong> ${escapeHtml(when)}</li>`;
    })
    .join("");
  const firstZone = Number(byKey.first_zone_created);
  const firstRun = Number(byKey.first_run);
  const ttv =
    firstZone && firstRun && firstRun >= firstZone
      ? `<p class="organizer-muted">Time to first run: ${Math.round((firstRun - firstZone) / 60)} minute(s) after the first Zone.</p>`
      : "";
  return `<ul class="organizer-history">${items}</ul>${ttv}`;
}

async function organizerShowHistory() {
  if (!invoke) return notAvailable();
  let history = [];
  let milestones = [];
  try {
    history = await invoke("organizer_list_executions");
  } catch (err) {
    return toastError("Could not load history: " + formatInvokeError(err));
  }
  try {
    milestones = await invoke("organizer_time_to_value");
  } catch {
    milestones = [];
  }
  const modal = document.getElementById("analysis-modal");
  const content = modal?.querySelector(".modal-content");
  if (!content) return;
  const zoneName = (id) => organizerZones.find((z) => z.id === id)?.name || id;
  const rows = history.length
    ? history
        .map(
          (h) => `<li>
            <div><strong>${escapeHtml(zoneName(h.zone_id))}</strong> — ${h.applied} applied, ${h.skipped} skipped, ${h.failed} failed ${h.sealed ? `<span class="org-seal" title="This run is sealed into the tamper-evident audit chain">${icon("i-seal")} sealed</span>` : ""}</div>
            <button class="btn btn--ghost btn--small" data-undo-exec="${escapeAttr(h.id)}">Undo</button>
          </li>`,
        )
        .join("")
    : "<li>No runs yet.</li>";
  content.innerHTML = `
    <h3 style="margin-top:0">Organizer history</h3>
    <div class="btn-row" style="margin-bottom:10px">
      <button class="btn btn--ghost btn--small" id="organizerVerifyBtn" type="button">Verify integrity</button>
      <button class="btn btn--ghost btn--small" id="organizerVerifyReportBtn" type="button">Verify a signed report…</button>
      <input type="file" id="organizerVerifyReportInput" accept=".txt,.md,text/plain" hidden />
    </div>
    <p class="organizer-verify-result" id="organizerVerifyResult" aria-live="polite"></p>
    <ul class="organizer-history">${rows}</ul>
    <h3 class="organizer-subhead">Milestones</h3>
    ${organizerRenderMilestones(milestones)}
    <div style="margin-top:16px"><button class="btn btn--ghost btn--small" data-close-modal="analysis-modal">Close</button></div>`;
  content.querySelectorAll("[data-undo-exec]").forEach((btn) =>
    btn.addEventListener("click", async () => {
      await organizerUndo(btn.dataset.undoExec);
      closeModal("analysis-modal");
    }),
  );
  const verifyBtn = document.getElementById("organizerVerifyBtn");
  if (verifyBtn) verifyBtn.addEventListener("click", organizerVerifyIntegrity);
  const verifyReportBtn = document.getElementById("organizerVerifyReportBtn");
  const verifyReportInput = document.getElementById("organizerVerifyReportInput");
  if (verifyReportBtn && verifyReportInput) {
    verifyReportBtn.addEventListener("click", () => verifyReportInput.click());
    verifyReportInput.addEventListener("change", () => {
      const file = verifyReportInput.files?.[0];
      verifyReportInput.value = "";
      if (file) organizerVerifySignedReport(file);
    });
  }
  showModal(modal);
}

// Verify a previously exported signed compliance report against this
// machine's signing key — the read side of organizer_export_audit's
// "signed" format. Confirms the report wasn't altered after export.
async function organizerVerifySignedReport(file) {
  if (!invoke) return notAvailable();
  const out = document.getElementById("organizerVerifyResult");
  if (out) out.textContent = "Verifying…";
  try {
    const text = await file.text();
    const ok = await invoke("organizer_verify_signed_report", { reportContent: text });
    if (!out) return;
    out.innerHTML = ok
      ? `<span class="org-verify org-verify--ok">${icon("i-check-circle")} Signature valid — report matches this machine's signing key</span>`
      : `<span class="org-verify org-verify--bad">${icon("i-x-circle")} Signature invalid — report was altered or signed on another machine</span>`;
  } catch (err) {
    if (out) out.textContent = "";
    toastError("Verify failed: " + formatInvokeError(err));
  }
}

// Verify the tamper-evident audit chain and render the verdict — the payoff of
// sealing each run: you can prove the local history wasn't altered, offline.
async function organizerVerifyIntegrity() {
  if (!invoke) return notAvailable();
  const out = document.getElementById("organizerVerifyResult");
  if (out) out.textContent = "Verifying…";
  let v;
  try {
    v = await invoke("organizer_verify_audit_chain");
  } catch (err) {
    if (out) out.textContent = "";
    return toastError("Verify failed: " + formatInvokeError(err));
  }
  if (!out) return;
  const unsealed =
    v.unsealed_count > 0 ? ` (${v.unsealed_count} older unsealed run(s) predate this feature)` : "";
  if (v.intact) {
    out.innerHTML = `<span class="org-verify org-verify--ok">${icon("i-check-circle")} Chain intact — ${v.sealed_count} run(s) sealed${escapeHtml(unsealed)}</span>`;
  } else {
    const at = v.first_break ? ` at run ${escapeHtml(v.first_break.execution_id)}` : "";
    const why = v.first_break ? ` — ${escapeHtml(v.first_break.reason)}` : "";
    out.innerHTML = `<span class="org-verify org-verify--bad">${icon("i-x-circle")} Tampering detected${at}${why}</span>`;
  }
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
    return toastError("Could not load replay history: " + formatInvokeError(err));
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
  // "why did this run behave that way" gets answered. When the run's event
  // list is the one still loaded, fallback rows name the semantic step
  // ("Clicked 'Send' in Mail") instead of a bare raw index.
  const traceHtml = (h) => {
    const trace = h.step_trace || [];
    if (!trace.length) return "";
    const eventsMatch = h.events_processed === recordedEvents.length && recordedEvents.length > 0;
    const count = (kind) => trace.filter((t) => t.kind === kind).length;
    const parts = [];
    if (count("RecordedPoint")) parts.push(`${count("RecordedPoint")} found in place`);
    if (count("WindowRelative")) parts.push(`${count("WindowRelative")} followed their window`);
    if (count("SpiralReresolved")) parts.push(`${count("SpiralReresolved")} re-resolved nearby`);
    if (count("TemplateMatched")) parts.push(`${count("TemplateMatched")} found by template match`);
    if (count("CoordinateFallback")) parts.push(`${count("CoordinateFallback")} lost their element`);
    if (count("NoDescriptor")) parts.push(`${count("NoDescriptor")} coordinate-only`);
    const risky = trace.filter(
      (t) => t.kind === "CoordinateFallback" || t.kind === "NoDescriptor"
    );
    const details = risky
      .map((t) => {
        const semantic = eventsMatch ? describeEvent(recordedEvents[t.step_index]) : null;
        const where = semantic ? escapeHtml(semantic) : `Step ${t.step_index + 1}`;
        const label = t.target_name
          ? `"${escapeHtml(t.target_name)}" not found — clicked recorded point`
          : "no element recorded — clicked fixed point";
        return `<div class="replay-meta replay-trace__fallback">${where}: ${label} (${t.point[0]}, ${t.point[1]})</div>`;
      })
      .join("");
    return `<div class="replay-meta">Targets: ${escapeHtml(parts.join(" · "))}</div>${details}`;
  };

  // Cross-run reliability — the north-star metric, computed from the records
  // already fetched (no extra command): success rate over finished runs, avg
  // duration, and how often clicks fell back to raw coordinates.
  const statsHtml = (() => {
    if (!history.length) return "";
    const finished = history.filter((h) => h.status === "Success" || h.status === "Failed");
    const parts = [`Last ${history.length} run(s)`];
    if (finished.length) {
      const ok = finished.filter((h) => h.status === "Success").length;
      parts.push(`${Math.round((ok / finished.length) * 100)}% success`);
    }
    const durations = history.map((h) => h.duration_ms).filter((d) => d || d === 0);
    if (durations.length) {
      parts.push(`avg ${fmtDur(durations.reduce((a, b) => a + b, 0) / durations.length)}`);
    }
    const clicks = history.flatMap((h) => h.step_trace || []);
    if (clicks.length) {
      const fell = clicks.filter(
        (t) => t.kind === "CoordinateFallback" || t.kind === "NoDescriptor"
      ).length;
      parts.push(`${Math.round((fell / clicks.length) * 100)}% of clicks on raw coordinates`);
    }
    return `<div class="replay-meta replay-stats">${escapeHtml(parts.join(" · "))}</div>`;
  })();

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
    ${statsHtml}
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
    // Hand the trace to the compressed-step review timeline so it can badge
    // the semantic steps that fell back during this run (same event list).
    window.__ghostLastReplayTrace = {
      eventCount: latest.events_processed,
      trace,
    };
    const lost = trace.filter((t) => t.kind === "CoordinateFallback").length;
    const blind = trace.filter((t) => t.kind === "NoDescriptor").length;
    const moved = trace.filter((t) => t.kind === "SpiralReresolved").length;
    const followed = trace.filter((t) => t.kind === "WindowRelative").length;
    if (lost + blind > 0) {
      // Name the first lost element semantically when the run matches the
      // loaded events — "which click" beats "how many clicks".
      const first = trace.find((t) => t.kind === "CoordinateFallback" || t.kind === "NoDescriptor");
      const semantic =
        latest.events_processed === recordedEvents.length && first
          ? describeEvent(recordedEvents[first.step_index])
          : null;
      showInsight(
        `${lost + blind} click(s) ran on raw coordinates` +
          (semantic ? ` (first: ${semantic})` : lost ? ` (${lost} couldn't find their element)` : "") +
          ". These steps are the most likely to break — open Replay History for the per-step trace."
      );
    } else if (moved + followed > 0) {
      showInsight(
        `Every click found its element — ${moved + followed} had moved and were re-resolved` +
          (followed ? ` (${followed} by following their window)` : " nearby") +
          "."
      );
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
  // A trace from a different workflow must not badge the new timeline.
  window.__ghostLastReplayTrace = null;
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
    return toastError("Could not build preview: " + formatInvokeError(err));
  }

  const modal = document.getElementById("analysis-modal");
  const content = modal?.querySelector(".modal-content");
  if (!content) return;

  const fallbackCount = steps.filter((s) => s.coordinate_fallback).length;
  const summary = fallbackCount
    ? `<p class="dry-run__warning"><strong>Needs review:</strong> ${fallbackCount} step(s) will click raw coordinates with no element to re-resolve — they break if the UI moves.</p>`
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
  // Review the full routine once at the start of a stepped run, then mint a
  // one-shot approval for each slice (fingerprint must match the events replayed).
  if (stepReplayCursor === 0 && !(await confirmPolicyBeforeReplay(recordedEvents))) return;

  const start = stepReplayCursor;
  const end = nextStepBoundary(recordedEvents, start);
  const slice = recordedEvents.slice(start, end);

  try {
    isPlaying = true;
    updateRecordingUI();
    // Mint a one-shot token for this exact slice (overwrites full-list approve from step 0).
    await invoke("approve_routine_replay", { events: slice });
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
    toastError("Step replay failed: " + formatInvokeError(error));
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
  const offset = lastFailedStep;
  const slice = recordedEvents.slice(offset);
  if (!(await confirmPolicyBeforeReplay(slice))) return;

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
    toastError("Retry failed: " + formatInvokeError(error));
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
  bind("deleteWorkflowBtn", deleteWorkflow);
  bind("reviewBtn", openWorkflowReview);
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
  bind("perm-recheck", refreshPermissionBanner);
  bind("perm-restart", restartApp);
  bind("settingsBtn", openSettings);
  bind("aboutBtn", openAbout);
  bind("lockBtn", lockApp);

  // Returning from System Settings re-checks. Accessibility (and often Input
  // Monitoring) usually still report false until Quit & Reopen — refresh still
  // updates banner copy toward the relaunch CTA when `_permAwaitingRelaunch`.
  window.addEventListener("focus", refreshPermissionBanner);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) refreshPermissionBanner();
  });

  // Ghost Organizer: the wedge product's trust pipeline.
  bind("organizerNewZoneBtn", organizerCreateZone);
  bind("organizerPresetBtn", organizerRunWizard);
  bind("organizerExportPackBtn", organizerExportPolicyPack);
  bind("organizerImportPackBtn", organizerImportPolicyPack);
  const importPackInput = document.getElementById("organizerImportPackInput");
  if (importPackInput) {
    importPackInput.addEventListener("change", () => {
      const file = importPackInput.files?.[0];
      importPackInput.value = "";
      if (file) organizerHandlePolicyPackFile(file);
    });
  }
  bind("organizerBrowseBtn", organizerBrowseFolder);
  bind("organizerAddFolderBtn", organizerAddFolder);
  bind("organizerScanBtn", organizerScan);
  bind("organizerAiSuggestBtn", organizerAiSuggest);
  bind("organizerMcpTokenBtn", organizerIssueMcpToken);
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
  bind("onboardingRestart", restartApp);
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
      return;
    }

    const signInTarget = e.target.closest("[data-account-sign-in]");
    if (signInTarget) {
      signInWithProvider(signInTarget.dataset.accountSignIn);
      return;
    }

    const signOutTarget = e.target.closest("[data-account-sign-out]");
    if (signOutTarget) {
      signOutAccount();
      return;
    }

    const intelTestTarget = e.target.closest("[data-intel-test]");
    if (intelTestTarget) {
      testIntelligenceProvider(intelTestTarget.dataset.intelTest);
      return;
    }

    if (e.target.closest("[data-intel-discover-local]")) {
      discoverLocalRuntimes();
      return;
    }

    if (e.target.closest("[data-power-bi-connect]")) {
      connectPowerBi();
      return;
    }
    if (e.target.closest("[data-power-bi-preview]")) {
      previewPowerBiExport();
      return;
    }
    if (e.target.closest("[data-power-bi-push]")) {
      pushPowerBiExport();
      return;
    }
    if (e.target.closest("[data-power-bi-revoke]")) {
      revokePowerBiGrant();
      return;
    }
    if (e.target.closest("[data-fabric-connect]")) {
      connectFabric();
      return;
    }
    if (e.target.closest("[data-fabric-list-workspaces]")) {
      listFabricWorkspaces();
      return;
    }
    if (e.target.closest("[data-fabric-preview]")) {
      previewFabricExport();
      return;
    }
    if (e.target.closest("[data-fabric-push]")) {
      pushFabricExport();
      return;
    }
    if (e.target.closest("[data-fabric-list-lakehouses]")) {
      listFabricLakehouses();
      return;
    }
    if (e.target.closest("[data-fabric-revoke]")) {
      revokeFabricGrant();
      return;
    }
    if (e.target.closest("[data-fabric-webhook-secret]")) {
      generateFabricWebhookSecret();
      return;
    }
    if (e.target.closest("[data-google-connect]")) {
      connectGoogleCloud();
      return;
    }
    if (e.target.closest("[data-google-list-buckets]")) {
      listGoogleBuckets();
      return;
    }
    if (e.target.closest("[data-google-bind-bucket]")) {
      bindGoogleExportBucket();
      return;
    }
    if (e.target.closest("[data-google-preview]")) {
      previewGoogleExport();
      return;
    }
    if (e.target.closest("[data-google-push]")) {
      pushGoogleExport();
      return;
    }
    if (e.target.closest("[data-google-revoke]")) {
      revokeGoogleCloudGrant();
      return;
    }
    if (e.target.closest("[data-mcp-enable-pairing]")) {
      enableMcpPairing();
      return;
    }
    if (e.target.closest("[data-mcp-disable-pairing]")) {
      disableMcpPairing();
      return;
    }
    if (e.target.closest("[data-mcp-http-start]")) {
      startMcpHttpServer();
      return;
    }
    if (e.target.closest("[data-mcp-http-stop]")) {
      stopMcpHttpServer();
      return;
    }
    if (e.target.closest("[data-mcp-relay-start]")) {
      startMcpRelay();
      return;
    }
    if (e.target.closest("[data-mcp-relay-stop]")) {
      stopMcpRelay();
      return;
    }
  });

  document.addEventListener("change", (e) => {
    if (e.target?.id === "fabric-workspace-id") {
      void listFabricLakehouses();
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
    toastError(`Update failed: ${formatInvokeError(err)}`);
  }
}

// The experimental tools panel drives commands that only exist when the backend
// is compiled with `--features experimental`. A stock build does not register
// them, so reveal the panel only when the backend confirms the surface is on.
// Fail closed: any error leaves the panel hidden rather than showing dead buttons.
const EXPERIMENTAL_ONLY_CONTROLS = [
  "replayReliableBtn",
  "analyzeBtn",
  "optimizeBtn",
  "organizerAiSuggestBtn",
];

async function initExperimentalPanel() {
  const panel = document.getElementById("experimentalPanel");
  if (!invoke) {
    experimentalEnabled = false;
    EXPERIMENTAL_ONLY_CONTROLS.forEach((id) =>
      document.getElementById(id)?.setAttribute("hidden", ""),
    );
    return;
  }
  try {
    experimentalEnabled = await invoke("is_experimental_enabled");
  } catch {
    experimentalEnabled = false;
  }
  if (experimentalEnabled) {
    panel?.removeAttribute("hidden");
    document.getElementById("navExperimental")?.removeAttribute("hidden");
    EXPERIMENTAL_ONLY_CONTROLS.forEach((id) =>
      document.getElementById(id)?.removeAttribute("hidden"),
    );
  } else {
    EXPERIMENTAL_ONLY_CONTROLS.forEach((id) =>
      document.getElementById(id)?.setAttribute("hidden", ""),
    );
  }
}

// Left-nav view switcher: one focused view visible at a time. Pure DOM, no
// backend — nav buttons and view sections are paired by `data-view`.
const seenMcpApprovalRequests = new Set();

async function pollMcpPendingApprovals() {
  if (!invoke) return;
  try {
    const pending = await invoke("mcp_list_pending_approvals");
    for (const req of pending || []) {
      if (!seenMcpApprovalRequests.has(req.request_id)) {
        seenMcpApprovalRequests.add(req.request_id);
        window.dispatchEvent(
          new CustomEvent("ghost:mcp-approval-request", { detail: req }),
        );
      }
    }
  } catch (_) {
    /* ignore polling errors */
  }
}

function initMcpApprovalPolling() {
  void pollMcpPendingApprovals();
  setInterval(pollMcpPendingApprovals, 5000);
}

function initViewNav() {
  const items = document.querySelectorAll(".nav__item");
  const views = document.querySelectorAll(".view");
  if (!items.length || !views.length) return;

  function show(view) {
    // Expose the active view so the sidebar can show only what's relevant to it.
    document.body.dataset.view = view;
    views.forEach((v) => {
      v.hidden = v.dataset.view !== view;
    });
    items.forEach((b) => {
      const active = b.dataset.view === view;
      b.classList.toggle("is-active", active);
      b.setAttribute("aria-selected", active ? "true" : "false");
    });
  }

  items.forEach((b) => {
    b.addEventListener("click", () => b.dataset.view && show(b.dataset.view));
  });

  window.addEventListener("ghost:mcp-approval-request", (event) => {
    const req = event.detail;
    show("organize");
    if (req?.zone_id) {
      organizerSelectedZoneId = req.zone_id;
      void organizerRefreshZones().then(() => {
        showNotification(
          "An MCP client requested plan approval — scan, review, then issue an MCP token.",
        );
      });
    }
  });

  show("organize");
}

// ===== Guard Desk (local OCR + compliance → POS Bridge) =====

const GUARD_SCENARIOS = {
  john_doe: {
    number: "1042",
    payee: "John Doe",
    amount: "1,450.00",
    memo: "Weekly Payroll",
    date: "2026-07-09",
    routing: "121000248",
    account: "987654321",
    signature: "J. R. Reynolds",
    idName: "JOHN DOE",
    idDob: "1992-08-14",
    idNumber: "DL-98234812",
    idExpiry: "2029-12-15",
    idIssue: "2021-12-15",
    idSex: "M",
    idClass: "C",
    idJurisdiction: "California",
    idDocType: "drivers_license",
    rules: [
      { text: "Payee matches ID Name", status: "pass" },
      { text: "ID Expiration Validity (Expires 2029)", status: "pass" },
      { text: "Check Date within 90 days limit", status: "pass" },
      { text: "Fraud & Signature Verification (Match: 98.4%)", status: "pass" },
      { text: "Store cashing limit check (< $3,000)", status: "pass" }
    ],
    verdict: "APPROVED",
    verdictClass: "success",
    recommendation: "<strong>Recommendation:</strong> Check matches all compliance criteria. Payee identity verified. Safe to cash. Approve, then use <em>Auto-fill POS</em> to type into the terminal.",
    code: "OK-88294"
  },
  jane_smith: {
    number: "5082",
    payee: "Jane Smith",
    amount: "4,500.00",
    memo: "Contractor Fee",
    date: "2026-07-08",
    routing: "021000021",
    account: "112233445",
    signature: "Sarah Jenkins",
    idName: "JANE SMITH",
    idDob: "1985-11-23",
    idNumber: "NY-88219421",
    idExpiry: "2030-05-18",
    idIssue: "2022-05-18",
    idSex: "F",
    idClass: "D",
    idJurisdiction: "New York",
    idDocType: "state_id",
    rules: [
      { text: "Payee matches ID Name", status: "pass" },
      { text: "ID Expiration Validity (Expires 2030)", status: "pass" },
      { text: "Check Date within 90 days limit", status: "pass" },
      { text: "Fraud & Signature Verification (Match: 95.1%)", status: "pass" },
      { text: "Store cashing limit check (> $3,000 threshold)", status: "warn" }
    ],
    verdict: "MANAGER REVIEW",
    verdictClass: "warn",
    recommendation: "<strong>Recommendation:</strong> Check amount ($4,500.00) exceeds teller authority limit of $3,000. Out-of-state routing number detected. Verify company credentials and request Manager override code.",
    code: "PENDING-LIMIT"
  },
  bad_check: {
    number: "1104",
    payee: "John Smith",
    amount: "850.00",
    memo: "Car Purchase",
    date: "2025-09-12",
    routing: "321070010",
    account: "556677889",
    signature: "Unknown Scribble",
    idName: "JOHN S. SMITH",
    idDob: "1978-04-02",
    idNumber: "IL-10492842",
    idExpiry: "2026-02-14",
    idIssue: "2018-02-14",
    idSex: "M",
    idClass: "C",
    idJurisdiction: "Illinois",
    idDocType: "drivers_license",
    rules: [
      { text: "Payee Name mismatch (ID shows John S. Smith)", status: "warn" },
      { text: "ID EXPIRED (Expired 2026-02-14)", status: "fail" },
      { text: "Check Date EXPIRED (> 90 days limit, Date: 2025-09-12)", status: "fail" },
      { text: "Signature mismatch (Match: 12.3% - Alert)", status: "fail" },
      { text: "Store cashing limit check", status: "pass" }
    ],
    verdict: "REJECTED",
    verdictClass: "error",
    recommendation: "<strong>Recommendation:</strong> Do not cash. Stale-dated check, expired ID, and failed signature check. POS Bridge stays locked.",
    code: "REJECT-BLOCKED"
  }
};

// --- Guard Desk OCR helpers (local Vision/OCR on macOS & Windows) ---

let guardCheckImageFile = null;
let guardIdImageFile = null;

function guardUpdateUploadStatus() {
  const el = document.getElementById("guardUploadStatus");
  if (!el) return;
  const parts = [];
  if (guardCheckImageFile) parts.push(`Check: ${guardCheckImageFile.name}`);
  if (guardIdImageFile) parts.push(`ID: ${guardIdImageFile.name}`);
  el.textContent = parts.length ? parts.join(" · ") : "No uploads — using preset";
}

function guardFileToBytes(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(Array.from(new Uint8Array(reader.result)));
    reader.onerror = () => reject(new Error("Could not read image file"));
    reader.readAsArrayBuffer(file);
  });
}

function guardOcrResultsToText(results) {
  if (!Array.isArray(results) || results.length === 0) return "";
  return results
    .map((r) => (r && r.text ? String(r.text).trim() : ""))
    .filter(Boolean)
    .join("\n");
}

function guardFirstMatch(text, patterns) {
  for (const re of patterns) {
    const m = text.match(re);
    if (m && m[1]) return m[1].trim();
  }
  return "";
}

function guardNormalizeName(name) {
  return String(name || "")
    .toUpperCase()
    .replace(/[^A-Z0-9 ]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function guardNamesSimilar(a, b) {
  const left = guardNormalizeName(a).split(" ").filter(Boolean);
  const right = guardNormalizeName(b).split(" ").filter(Boolean);
  if (!left.length || !right.length) return false;
  const overlap = left.filter((tok) => right.includes(tok));
  return overlap.length >= Math.min(left.length, right.length) - 1;
}

function guardParseDate(value) {
  if (!value) return null;
  const iso = value.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (iso) return new Date(`${iso[1]}-${iso[2]}-${iso[3]}T12:00:00`);
  const slash = value.match(/^(\d{1,2})\/(\d{1,2})\/(\d{2,4})$/);
  if (slash) {
    const year = slash[3].length === 2 ? `20${slash[3]}` : slash[3];
    return new Date(`${year}-${slash[1].padStart(2, "0")}-${slash[2].padStart(2, "0")}T12:00:00`);
  }
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function guardParseCheckText(text) {
  const upper = text.toUpperCase();
  const payee = guardFirstMatch(upper, [
    /PAY\s+TO\s+(?:THE\s+)?ORDER\s+OF\s*[:.]?\s*([A-Z][A-Z .'-]+)/,
    /PAYEE\s*[:.]?\s*([A-Z][A-Z .'-]+)/,
  ]);
  const amountRaw = guardFirstMatch(text, [
    /\$\s*([\d,]+\.\d{2})/,
    /AMOUNT\s*[:.]?\s*\$?\s*([\d,]+\.\d{2})/i,
  ]);
  const amount = amountRaw ? amountRaw.replace(/,/g, "") : "";
  const memo = guardFirstMatch(text, [/MEMO\s*[:.]?\s*(.+)/i]);
  const date = guardFirstMatch(text, [
    /DATE\s*[:.]?\s*(\d{4}-\d{2}-\d{2})/i,
    /DATE\s*[:.]?\s*(\d{1,2}\/\d{1,2}\/\d{2,4})/i,
    /\b(\d{4}-\d{2}-\d{2})\b/,
  ]);
  const routing = guardFirstMatch(text, [/(?:⑆|ROUTING|RTN)\s*(\d{9})/i, /\b(\d{9})\b/]);
  const accountPatterns = [/ACCOUNT\s*[:.]?\s*(\d{6,17})/i, /\b(\d{8,17})\b/];
  if (routing) accountPatterns.unshift(new RegExp(`${routing}\\D+(\\d{6,17})`));
  const account = guardFirstMatch(text, accountPatterns);
  const signature = guardFirstMatch(text, [
    /SIGN(?:ATURE)?\s*[:.]?\s*([A-Za-z .'-]{2,40})/i,
    /([A-Z]\.\s*[A-Z]\.\s+[A-Za-z'-]+)/,
  ]);
  const number = guardFirstMatch(text, [/DOC(?:UMENT)?\s*(?:NO|#)?\s*[:.]?\s*(\d{3,6})/i, /\bNO\.?\s*(\d{3,6})\b/i]);
  return {
    number: number || "—",
    payee: payee || "Unknown Payee",
    amount: amount ? Number(amount).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : "0.00",
    memo: memo || "—",
    date: date || "—",
    routing: routing || "—",
    account: account || "—",
    signature: signature || "—",
  };
}

// Fallback ID parser used only when the deterministic Rust scanner
// (`parse_id_document`) is unavailable (e.g. running outside the desktop app).
// The backend `guardScanId` is the primary, unit-tested path.
function guardParseIdText(text) {
  const idName = guardFirstMatch(text, [
    /NAME\s*[:.]?\s*([A-Z][A-Z .'-]+)/,
    /FULL\s+NAME\s*[:.]?\s*([A-Z][A-Z .'-]+)/,
  ]);
  const idDob = guardFirstMatch(text, [
    /(?:DOB|DATE\s+OF\s+BIRTH)\s*[:.]?\s*(\d{4}-\d{2}-\d{2})/i,
    /(?:DOB|DATE\s+OF\s+BIRTH)\s*[:.]?\s*(\d{1,2}\/\d{1,2}\/\d{2,4})/i,
  ]);
  const idNumber = guardFirstMatch(text, [
    /(?:ID\s*(?:NO|#)?|LICENSE|DL)\s*[:.]?\s*([A-Z0-9-]+)/i,
  ]);
  const idExpiry = guardFirstMatch(text, [
    /(?:EXP(?:IRES|IRATION)?)\s*[:.]?\s*(\d{4}-\d{2}-\d{2})/i,
    /(?:EXP(?:IRES|IRATION)?)\s*[:.]?\s*(\d{1,2}\/\d{1,2}\/\d{2,4})/i,
  ]);
  return {
    idName: idName || "UNKNOWN",
    idDob: idDob || "—",
    idNumber: idNumber || "—",
    idExpiry: idExpiry || "—",
  };
}

// Human-readable label for the backend `documentType` enum value.
function guardDocTypeLabel(docType) {
  switch (docType) {
    case "drivers_license":
      return "Driver License";
    case "state_id":
      return "State ID";
    case "passport":
      return "Passport";
    default:
      return "ID Document";
  }
}

// Map the backend IdScan (camelCase) onto the flat fields the Guard Desk UI and
// compliance evaluator use.
function guardIdScanToFields(scan) {
  return {
    idName: scan.name || "UNKNOWN",
    idDob: scan.dob || "—",
    idNumber: scan.idNumber || "—",
    idExpiry: scan.expiry || "—",
    idIssue: scan.issueDate || "—",
    idAddress: scan.address || "—",
    idSex: scan.sex || "—",
    idClass: scan.class || "—",
    idJurisdiction: scan.jurisdiction || "—",
    idDocType: scan.documentType || "unknown",
    idAge: typeof scan.age === "number" ? scan.age : null,
    idExpired: typeof scan.expired === "boolean" ? scan.expired : null,
    idExpiresSoon: typeof scan.expiresSoon === "boolean" ? scan.expiresSoon : null,
    idFlags: Array.isArray(scan.flags) ? scan.flags : [],
  };
}

// Run the deterministic backend ID scanner over OCR text, falling back to the
// in-page JS parser if the command is missing or errors.
async function guardScanId(idText) {
  if (invoke) {
    try {
      const scan = await invoke("parse_id_document", { text: idText });
      return guardIdScanToFields(scan);
    } catch (err) {
      console.warn("parse_id_document failed, using JS fallback:", err);
    }
  }
  return guardParseIdText(idText);
}

function guardSignatureScore(payee, signature) {
  const payeeTokens = guardNormalizeName(payee).split(" ").filter(Boolean);
  const sigTokens = guardNormalizeName(signature).split(" ").filter(Boolean);
  if (!payeeTokens.length || !sigTokens.length || signature === "—") return 0;
  const hits = payeeTokens.filter((t) => sigTokens.some((s) => s.startsWith(t[0]) || s.includes(t)));
  return Math.round((hits.length / payeeTokens.length) * 100);
}

function guardEvaluateCompliance(raw) {
  const today = new Date();
  const rules = [];
  let failCount = 0;
  let warnCount = 0;

  const payeeNorm = guardNormalizeName(raw.payee);
  const idNorm = guardNormalizeName(raw.idName);
  if (payeeNorm && idNorm && payeeNorm === idNorm) {
    rules.push({ text: "Payee matches ID Name", status: "pass" });
  } else if (payeeNorm && idNorm && guardNamesSimilar(raw.payee, raw.idName)) {
    rules.push({ text: `Payee Name mismatch (ID shows ${raw.idName})`, status: "warn" });
    warnCount += 1;
  } else {
    rules.push({ text: `Payee Name mismatch (ID shows ${raw.idName || "unknown"})`, status: "fail" });
    failCount += 1;
  }

  const expiry = guardParseDate(raw.idExpiry);
  const backendExpired = raw.idExpired === true;
  const backendExpiresSoon = raw.idExpiresSoon === true;
  if (backendExpired || (expiry && expiry < today)) {
    rules.push({ text: `ID EXPIRED (Expired ${raw.idExpiry})`, status: "fail" });
    failCount += 1;
  } else if (expiry) {
    if (backendExpiresSoon) {
      rules.push({ text: `ID expires soon (${raw.idExpiry})`, status: "warn" });
      warnCount += 1;
    } else {
      rules.push({ text: `ID Expiration Validity (Expires ${raw.idExpiry})`, status: "pass" });
    }
  } else {
    rules.push({ text: "ID Expiration Validity (date not detected)", status: "warn" });
    warnCount += 1;
  }

  // Age check from the ID scan: cashing to a minor is a hard stop.
  if (typeof raw.idAge === "number") {
    if (raw.idAge < 18) {
      rules.push({ text: `Cardholder age check (MINOR — age ${raw.idAge})`, status: "fail" });
      failCount += 1;
    } else {
      rules.push({ text: `Cardholder age check (age ${raw.idAge})`, status: "pass" });
    }
  }

  // Surface any remaining scanner review flags (low-confidence / missing fields)
  // that did not already map to a specific rule above.
  if (Array.isArray(raw.idFlags)) {
    const covered = /expired|expires within|minor/i;
    raw.idFlags
      .filter((f) => !covered.test(f))
      .forEach((f) => {
        rules.push({ text: `ID scan: ${f}`, status: "warn" });
        warnCount += 1;
      });
  }

  const checkDate = guardParseDate(raw.date);
  if (checkDate) {
    const ageDays = Math.floor((today - checkDate) / (1000 * 60 * 60 * 24));
    if (ageDays <= 90) {
      rules.push({ text: `Check Date within 90 days limit (${raw.date})`, status: "pass" });
    } else {
      rules.push({ text: `Check Date EXPIRED (> 90 days limit, Date: ${raw.date})`, status: "fail" });
      failCount += 1;
    }
  } else {
    rules.push({ text: "Check Date within 90 days limit (date not detected)", status: "warn" });
    warnCount += 1;
  }

  const sigScore = guardSignatureScore(raw.payee, raw.signature);
  if (sigScore >= 60) {
    rules.push({ text: `Fraud & Signature Verification (Match: ${sigScore}.0%)`, status: "pass" });
  } else if (sigScore >= 30) {
    rules.push({ text: `Fraud & Signature Verification (Match: ${sigScore}.0% - Review)`, status: "warn" });
    warnCount += 1;
  } else {
    rules.push({ text: `Signature mismatch (Match: ${sigScore}.0% - Alert)`, status: "fail" });
    failCount += 1;
  }

  const amountNum = parseFloat(String(raw.amount).replace(/,/g, ""));
  if (!Number.isNaN(amountNum) && amountNum <= 3000) {
    rules.push({ text: "Store cashing limit check (< $3,000)", status: "pass" });
  } else if (!Number.isNaN(amountNum)) {
    rules.push({ text: `Store cashing limit check (> $3,000 threshold)`, status: "warn" });
    warnCount += 1;
  } else {
    rules.push({ text: "Store cashing limit check (amount not detected)", status: "warn" });
    warnCount += 1;
  }

  let verdict;
  let verdictClass;
  let recommendation;
  let code;
  if (failCount > 0) {
    verdict = "REJECTED";
    verdictClass = "error";
    code = "REJECT-OCR";
    recommendation =
      "<strong>Recommendation:</strong> Do not cash. One or more compliance checks failed on OCR-parsed documents. Review or rescan with clearer images.";
  } else if (warnCount > 0) {
    verdict = "MANAGER REVIEW";
    verdictClass = "warn";
    code = "PENDING-OCR";
    recommendation =
      "<strong>Recommendation:</strong> OCR parsed the documents but flagged items for manager review. Verify limits, name alignment, and routing before cashing.";
  } else {
    verdict = "APPROVED";
    verdictClass = "success";
    code = "OK-OCR";
    recommendation =
      "<strong>Recommendation:</strong> Check matches all compliance criteria from local OCR. Payee identity verified. Safe to cash. Approve, then use <em>Auto-fill POS</em> to type into the terminal.";
  }

  return { ...raw, rules, verdict, verdictClass, recommendation, code };
}

async function guardScanFromImages(checkFile, idFile) {
  if (!invoke) throw new Error("OCR requires the Ghost desktop app");
  let checkText = "";
  let idText = "";

  if (checkFile) {
    const bytes = await guardFileToBytes(checkFile);
    const results = await invoke("run_ocr_on_image", { imageBytes: bytes });
    checkText = guardOcrResultsToText(results);
    if (!checkText) throw new Error("No text recognized on check image");
  }
  let idFields = null;
  if (idFile) {
    const bytes = await guardFileToBytes(idFile);
    const results = await invoke("run_ocr_on_image", { imageBytes: bytes });
    idText = guardOcrResultsToText(results);
    if (!idText) throw new Error("No text recognized on ID image");
    idFields = await guardScanId(idText);
  }

  const parsed = {
    ...guardParseCheckText(checkText),
    ...(idFields || guardParseIdText(idText)),
  };
  return guardEvaluateCompliance(parsed);
}

function guardPopulateVisuals(data) {
  document.getElementById("chkNumber").textContent = data.number;
  document.getElementById("chkPayee").textContent = data.payee;
  document.getElementById("chkAmount").textContent = data.amount;
  document.getElementById("chkMemo").textContent = data.memo;
  document.getElementById("chkDate").textContent = data.date;
  document.getElementById("chkRouting").textContent = data.routing;
  document.getElementById("chkAccount").textContent = data.account;
  document.getElementById("chkSignature").textContent = data.signature;
  document.getElementById("idName").textContent = data.idName;
  document.getElementById("idDob").textContent = data.idDob;
  document.getElementById("idNumber").textContent = data.idNumber;
  document.getElementById("idExpiry").textContent = data.idExpiry;

  // Extended ID-scan fields (present on OCR scans and enriched presets; older
  // preset data leaves them undefined, so default to an em dash).
  const setText = (id, val) => {
    const el = document.getElementById(id);
    if (el) el.textContent = val || "—";
  };
  setText("idIssue", data.idIssue);
  setText("idSex", data.idSex);
  setText("idClass", data.idClass);
  setText("idJurisdiction", data.idJurisdiction);
  const docTypeEl = document.getElementById("idDocType");
  if (docTypeEl) docTypeEl.textContent = guardDocTypeLabel(data.idDocType);

  const flagsEl = document.getElementById("guardIdFlags");
  if (flagsEl) {
    const flags = Array.isArray(data.idFlags) ? data.idFlags : [];
    flagsEl.textContent = flags.length ? `⚠ ${flags.join(" · ")}` : "";
    flagsEl.style.display = flags.length ? "block" : "none";
  }
}

function guardDeskInit() {
  const scanBtn = document.getElementById("guardScanBtn");
  const selectEl = document.getElementById("guardScenarioSelect");
  const visualContainer = document.getElementById("guardVisualContainer");
  const analysisResult = document.getElementById("guardAnalysisResult");
  const autofillBtn = document.getElementById("guardAutofillBtn");
  const resetBtn = document.getElementById("guardResetBtn");
  const checkInput = document.getElementById("guardCheckImageInput");
  const idInput = document.getElementById("guardIdImageInput");
  const uploadCheckBtn = document.getElementById("guardUploadCheckBtn");
  const uploadIdBtn = document.getElementById("guardUploadIdBtn");
  
  if (!scanBtn) return;

  guardUpdateUploadStatus();

  uploadCheckBtn?.addEventListener("click", () => checkInput?.click());
  uploadIdBtn?.addEventListener("click", () => idInput?.click());

  checkInput?.addEventListener("change", () => {
    guardCheckImageFile = checkInput.files?.[0] || null;
    guardUpdateUploadStatus();
  });
  idInput?.addEventListener("change", () => {
    guardIdImageFile = idInput.files?.[0] || null;
    guardUpdateUploadStatus();
  });
  
  scanBtn.addEventListener("click", async () => {
    if (guardScanInProgress) return;
    guardScanInProgress = true;
    scanBtn.disabled = true;

    // 1. Reset POS and visual state
    guardResetPosForm();
    analysisResult.style.display = "none";
    autofillBtn.disabled = true;

    // 2. Load scenario — real OCR when uploads exist, preset otherwise
    let data;
    const useOcr = guardCheckImageFile || guardIdImageFile;
    if (useOcr) {
      try {
        data = await guardScanFromImages(guardCheckImageFile, guardIdImageFile);
        showNotification("Local OCR scan complete", "info");
      } catch (err) {
        console.warn("OCR failed:", err);
        toastError(
          "Could not read the uploaded image(s). Try a clearer photo or use a demo preset without uploads.",
        );
        guardScanInProgress = false;
        scanBtn.disabled = false;
        return;
      }
    } else {
      data = GUARD_SCENARIOS[selectEl.value];
    }

    guardPopulateVisuals(data);
    guardLastScanData = data;

    // 3. Show visual container
    visualContainer.style.display = "flex";

    // 4. Run scan animations
    const checkBar = document.getElementById("guardCheckScanBar");
    const idBar = document.getElementById("guardIdScanBar");

    checkBar.style.display = "block";
    idBar.style.display = "block";

    checkBar.style.top = "-4px";
    idBar.style.top = "-4px";

    checkBar.offsetHeight;
    idBar.offsetHeight;

    checkBar.style.top = "100%";
    idBar.style.top = "100%";

    setTimeout(() => {
      checkBar.style.display = "none";
      idBar.style.display = "none";
      guardShowComplianceResults(data);
      guardScanInProgress = false;
      scanBtn.disabled = false;
    }, 2000);
  });
  
  autofillBtn.addEventListener("click", () => {
    const data = guardLastScanData || GUARD_SCENARIOS[selectEl.value];
    if (!data) {
      toastError("Run a scan first");
      return;
    }
    guardRunAutofillReplay(data);
  });
  
  resetBtn.addEventListener("click", () => {
    guardResetPosForm();
    visualContainer.style.display = "none";
    analysisResult.style.display = "none";
    document.getElementById("guardReplayLogContainer").style.display = "none";
    autofillBtn.disabled = true;
    guardCheckImageFile = null;
    guardIdImageFile = null;
    guardLastScanData = null;
    if (checkInput) checkInput.value = "";
    if (idInput) idInput.value = "";
    guardUpdateUploadStatus();
  });
}

function guardShowComplianceResults(data) {
  const resultPanel = document.getElementById("guardAnalysisResult");
  const verdictEl = document.getElementById("guardVerdict");
  const recEl = document.getElementById("guardRecommendation");
  const autofillBtn = document.getElementById("guardAutofillBtn");
  
  verdictEl.textContent = data.verdict;
  
  // Clear styles
  verdictEl.style.backgroundColor = "transparent";
  verdictEl.style.color = "#fff";
  
  if (data.verdictClass === "success") {
    verdictEl.style.backgroundColor = "rgba(131, 246, 196, 0.15)";
    verdictEl.style.color = "var(--success)";
    verdictEl.style.borderColor = "var(--success)";
  } else if (data.verdictClass === "warn") {
    verdictEl.style.backgroundColor = "rgba(255, 184, 107, 0.15)";
    verdictEl.style.color = "var(--accent-warm)";
    verdictEl.style.borderColor = "var(--accent-warm)";
  } else {
    verdictEl.style.backgroundColor = "rgba(239, 68, 68, 0.15)";
    verdictEl.style.color = "#ef4444";
    verdictEl.style.borderColor = "#ef4444";
  }
  
  recEl.innerHTML = data.recommendation;

  // Render rules dynamically so ID-scan checks (age, expiry-soon, review flags)
  // can extend the list beyond the original fixed five.
  const ruleList = document.getElementById("guardRuleList");
  if (ruleList) {
    ruleList.innerHTML = "";
    (data.rules || []).forEach((rule) => {
      const li = document.createElement("li");
      li.style.display = "flex";
      li.style.alignItems = "center";
      li.style.gap = "10px";
      li.style.padding = "2px 0";
      const icon = document.createElement("span");
      if (rule.status === "pass") {
        icon.textContent = "✅";
        li.style.color = "var(--text)";
      } else if (rule.status === "warn") {
        icon.textContent = "⚠️";
        li.style.color = "var(--accent-warm)";
      } else {
        icon.textContent = "❌";
        li.style.color = "#ef4444";
      }
      li.appendChild(icon);
      li.appendChild(document.createTextNode(" " + rule.text));
      ruleList.appendChild(li);
    });
  }
  
  resultPanel.style.display = "block";
  
  // Trust pipeline: recommend → human approve → POS Bridge keys.
  const approveBtn = document.getElementById("guardApproveBtn");
  if (approveBtn) {
    approveBtn.disabled = data.verdict === "REJECTED";
    approveBtn.style.display = data.verdict === "REJECTED" ? "none" : "inline-flex";
    approveBtn.onclick = () => {
      autofillBtn.disabled = false;
      approveBtn.disabled = true;
      approveBtn.textContent = "Approved";
      showNotification("Approved. POS Bridge can auto-fill the terminal.", "info");
    };
    if (data.verdict !== "REJECTED") {
      approveBtn.textContent = "Approve plan";
      approveBtn.disabled = false;
    }
  }
  autofillBtn.disabled = true;
}

function guardResetPosForm() {
  document.getElementById("posPayeeName").value = "";
  document.getElementById("posIdNumber").value = "";
  document.getElementById("posCheckAmount").value = "";
  document.getElementById("posRoutingNumber").value = "";
  document.getElementById("posAccountNumber").value = "";
  document.getElementById("posVerifyStatus").value = "";
  
  document.getElementById("posPayeeName").style.borderColor = "var(--border)";
  document.getElementById("posIdNumber").style.borderColor = "var(--border)";
  document.getElementById("posCheckAmount").style.borderColor = "var(--border)";
  document.getElementById("posRoutingNumber").style.borderColor = "var(--border)";
  document.getElementById("posAccountNumber").style.borderColor = "var(--border)";
  document.getElementById("posVerifyStatus").style.borderColor = "var(--border)";
  
  document.getElementById("guardPosForm").style.opacity = "0.8";
}

async function guardRunAutofillReplay(data) {
  const autofillBtn = document.getElementById("guardAutofillBtn");
  const logContainer = document.getElementById("guardReplayLogContainer");
  const logBox = document.getElementById("guardReplayLog");
  
  autofillBtn.disabled = true;
  logContainer.style.display = "block";
  logBox.innerHTML = "";
  
  document.getElementById("guardPosForm").style.opacity = "1";
  
  const appendLog = (msg) => {
    const div = document.createElement("div");
    div.textContent = msg;
    logBox.appendChild(div);
    logBox.scrollTop = logBox.scrollHeight;
  };
  
  const typeField = (inputId, value) => {
    return new Promise((resolve) => {
      const input = document.getElementById(inputId);
      input.style.borderColor = "var(--accent)";
      appendLog(`[GHOST] Replaying click at target: #${inputId}`);
      
      let index = 0;
      const timer = setInterval(() => {
        if (index < value.length) {
          input.value += value.charAt(index);
          index++;
        } else {
          clearInterval(timer);
          input.style.borderColor = "var(--border)";
          appendLog(`[GHOST] Keystrokes complete for #${inputId}`);
          resolve();
        }
      }, 40);
    });
  };
  
  appendLog("[GHOST] Initiating POS Bridge Auto-Fill Routine...");
  await sleep(600);
  
  // Field 1: Payee Name
  await typeField("posPayeeName", data.idName);
  await sleep(400);
  
  // Field 2: ID Number
  await typeField("posIdNumber", data.idNumber);
  await sleep(400);
  
  // Field 3: Check Amount
  await typeField("posCheckAmount", data.amount);
  await sleep(400);
  
  // Field 4: Routing Number
  await typeField("posRoutingNumber", data.routing);
  await sleep(400);
  
  // Field 5: Account Number
  await typeField("posAccountNumber", data.account);
  await sleep(500);
  
  // Field 6: Verification Code
  const statusInput = document.getElementById("posVerifyStatus");
  statusInput.style.borderColor = "var(--accent)";
  appendLog(`[GHOST] Guard Desk checking transaction integrity...`);
  await sleep(800);
  appendLog(`[GHOST] Compliance integrity check PASSED.`);
  await typeField("posVerifyStatus", data.code);
  statusInput.style.borderColor = "var(--success)";
  
  appendLog("[GHOST] Replay finished successfully.");
  showNotification("Ghost Auto-Fill completed successfully!", "info");
  autofillBtn.disabled = false;
}

// ===== Plan Filing (audience-aware, read-only filing preview + savings) =====
// Wires the safe-read `preview_file_filing` and `estimate_filing_savings`
// commands. Preview reasons only over the file *names* the user pastes — it
// never reads the disk. Applying changes still goes through the Organizer's
// approve -> audit -> undo pipeline.

const FILING_SAMPLES = {
  engineering: [
    "junit-test-results-2026-07.xml",
    "coverage-Q2-2026.html",
    "build-log-2026-07-10.txt",
    "playwright-screenshot-checkout-2026-07.png",
    "incident-postmortem-2026-06.md",
    "vacation.jpg",
  ],
  finance: [
    "Q2 2026 Income Statement.xlsx",
    "Balance Sheet 2026-06.xlsx",
    "Cash Flow Statement Q1 2026.xlsx",
    "Trial Balance 2026-06.csv",
    "Invoice 10432 Acme 2026-05-14.pdf",
    "vacation.jpg",
  ],
  student: [
    "CS101 Fall 2026 homework 3.pdf",
    "MATH204 midterm study guide.docx",
    "final project proposal.docx",
    "ENG210 essay draft 2026-03.docx",
    "lab report week 5.pdf",
  ],
};

function filingParseNames(raw) {
  return String(raw || "")
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function filingNum(id) {
  const el = document.getElementById(id);
  if (!el) return null;
  const raw = el.value.trim();
  if (raw === "") return null;
  const n = Number(raw);
  return Number.isFinite(n) ? n : null;
}

function filingMoney(v) {
  if (v == null) return null;
  return `$${Number(v).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

function filingConfidenceBadge(item) {
  if (!item.recognized) return `<span class="org-badge org-badge--deny">unsorted</span>`;
  if (item.needs_review) return `<span class="org-badge org-badge--confirm">review</span>`;
  return `<span class="org-badge org-badge--allow">${Math.round((item.confidence || 0) * 100)}%</span>`;
}

async function filingPreview() {
  if (!invoke) return notAvailable();
  const audience = document.getElementById("filingAudience")?.value || "engineering";
  const root = document.getElementById("filingRoot")?.value || "";
  const fileNames = filingParseNames(document.getElementById("filingNames")?.value);
  const result = document.getElementById("filingResult");
  if (!result) return;
  if (!fileNames.length) {
    result.innerHTML = `<div class="organizer-summary">Paste one or more file names above to preview how they would be filed.</div>`;
    return;
  }
  const previewBtn = document.getElementById("filingPreviewBtn");
  const prevLabel = previewBtn?.textContent;
  if (previewBtn) {
    previewBtn.disabled = true;
    previewBtn.textContent = "Previewing…";
  }
  let preview;
  try {
    preview = await invoke("preview_file_filing", { audience, root, fileNames });
  } catch (err) {
    toastError("Filing preview failed: " + formatInvokeError(err));
    if (previewBtn) {
      previewBtn.disabled = false;
      previewBtn.textContent = prevLabel || "Preview filing";
    }
    return;
  } finally {
    if (previewBtn) {
      previewBtn.disabled = false;
      previewBtn.textContent = prevLabel || "Preview filing";
    }
  }
  const rows = (preview.items || [])
    .map(
      (it) => `<tr>
        <td><code>${escapeHtml(it.file_name)}</code></td>
        <td>${escapeHtml(it.category)}${it.period ? ` <span style="color:var(--muted)">· ${escapeHtml(it.period)}</span>` : ""}</td>
        <td><code>${escapeHtml(it.relative_dir)}</code></td>
        <td>${filingConfidenceBadge(it)}</td>
      </tr>`,
    )
    .join("");
  const cats = `${preview.distinct_categories} categor${preview.distinct_categories === 1 ? "y" : "ies"}`;
  const periods = `${preview.distinct_periods} period${preview.distinct_periods === 1 ? "" : "s"}`;
  result.innerHTML = `
    <div class="organizer-summary">
      Filing under <code>${escapeHtml(preview.root)}</code> ·
      <strong>${preview.recognized}</strong> recognized ·
      <strong>${preview.needs_review}</strong> need review ·
      <strong>${preview.unrecognized}</strong> unsorted · ${cats}, ${periods}
    </div>
    <table class="organizer-table">
      <thead><tr><th>File</th><th>Category</th><th>Proposed folder</th><th>Confidence</th></tr></thead>
      <tbody>${rows}</tbody>
    </table>
    <p class="panel__hint">Preview only — no files were moved. Apply changes in the Organizer, which requires approval and writes an undo journal.</p>`;
}

async function filingEstimateSavings() {
  if (!invoke) return notAvailable();
  const result = document.getElementById("savingsResult");
  if (!result) return;
  const estimateBtn = document.getElementById("savingsEstimateBtn");
  const prevLabel = estimateBtn?.textContent;
  if (estimateBtn) {
    estimateBtn.disabled = true;
    estimateBtn.textContent = "Estimating…";
  }
  const errorPct = filingNum("savingsErrorRate");
  const inputs = {
    files_per_period: filingNum("savingsFiles") ?? 0,
    periods_per_year: filingNum("savingsPeriods") ?? 0,
    minutes_per_file_manual: filingNum("savingsManual") ?? 0,
    minutes_per_file_assisted: filingNum("savingsAssisted"),
    hourly_rate: filingNum("savingsRate"),
    manual_error_rate: errorPct == null ? null : errorPct / 100,
    minutes_per_error_rework: null,
  };
  let est;
  try {
    est = await invoke("estimate_filing_savings", { inputs });
  } catch (err) {
    toastError("Savings estimate failed: " + formatInvokeError(err));
    if (estimateBtn) {
      estimateBtn.disabled = false;
      estimateBtn.textContent = prevLabel || "Estimate savings";
    }
    return;
  } finally {
    if (estimateBtn) {
      estimateBtn.disabled = false;
      estimateBtn.textContent = prevLabel || "Estimate savings";
    }
  }
  const costLine =
    est.cost_saved_per_year != null
      ? ` · <strong>${filingMoney(est.cost_saved_per_year)}</strong>/yr saved`
      : "";
  const manualCost = est.manual_cost_per_year != null ? ` (${filingMoney(est.manual_cost_per_year)})` : "";
  const assistedCost =
    est.assisted_cost_per_year != null ? ` (${filingMoney(est.assisted_cost_per_year)})` : "";
  const assumptions = (est.assumptions || []).map((a) => `<li>${escapeHtml(a)}</li>`).join("");
  result.innerHTML = `
    <div class="organizer-summary organizer-summary--done">
      ✓ <strong>${est.hours_saved_per_year}</strong> hours/yr saved (${est.reduction_pct}% less time)${costLine}
    </div>
    <table class="organizer-table">
      <tbody>
        <tr><th>Files per year</th><td>${est.files_per_year}</td></tr>
        <tr><th>Manual time</th><td>${est.manual_hours_per_year} h/yr${manualCost}</td></tr>
        <tr><th>Assisted time</th><td>${est.assisted_hours_per_year} h/yr${assistedCost}</td></tr>
        <tr><th>Rework hours avoided</th><td>${est.rework_hours_avoided_per_year} h/yr</td></tr>
      </tbody>
    </table>
    <h3 class="organizer-subhead">Assumptions</h3>
    <ul class="organizer-audit">${assumptions}</ul>`;
}

function filingInit() {
  const previewBtn = document.getElementById("filingPreviewBtn");
  if (!previewBtn) return;
  previewBtn.addEventListener("click", filingPreview);
  document.getElementById("filingSampleBtn")?.addEventListener("click", () => {
    const audience = document.getElementById("filingAudience")?.value || "engineering";
    const ta = document.getElementById("filingNames");
    if (ta) ta.value = (FILING_SAMPLES[audience] || []).join("\n");
  });
  document.getElementById("filingClearBtn")?.addEventListener("click", () => {
    const ta = document.getElementById("filingNames");
    if (ta) ta.value = "";
    const result = document.getElementById("filingResult");
    if (result) result.innerHTML = "";
  });
  document.getElementById("savingsEstimateBtn")?.addEventListener("click", filingEstimateSavings);
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
  initModalEscape();
  initViewNav(); // left-nav view switcher
  initMcpApprovalPolling();
  guardDeskInit(); // Guard Desk + POS Bridge
  filingInit(); // Plan Filing: read-only filing preview + savings estimate
});
