// Ghost desktop app — Tauri IPC integration, recording controls, workflow
// management, cloud sync, and Smart Observer. This is the real app UI
// (not the marketing site — that lives in public/).

const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};

function notAvailable() {
  alert("Tauri not available - running in static mode");
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

function showNotification(text) {
  const notificationsEl = document.getElementById("notifications");
  if (!notificationsEl) return;

  const notification = document.createElement("div");
  notification.className = "notification notification--proactive";
  notification.innerHTML = `<p class="notification__text">🦜 ${text}</p>`;
  notificationsEl.appendChild(notification);

  setTimeout(() => notification.remove(), 5000);
}

// ===== Accessibility permission gate =====

async function refreshPermissionBanner() {
  if (!invoke) return;

  const banner = document.getElementById("perm-banner");
  if (!banner) return;

  try {
    const granted = await invoke("check_accessibility");
    banner.hidden = granted;
  } catch (error) {
    console.error("Failed to check accessibility permission:", error);
  }
}

async function requestAccessibility() {
  if (!invoke) return;
  try {
    await invoke("request_accessibility");
  } catch (error) {
    console.error("Failed to request accessibility permission:", error);
  } finally {
    refreshPermissionBanner();
  }
}

// ===== First-run onboarding =====

const ONBOARDING_KEY = "ghost.onboarding.completed";
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

  // The permission step (index 1) needs live status polling.
  if (n === 1) {
    refreshOnboardingPermStatus();
    startPermPolling();
  } else {
    stopPermPolling();
  }
}

async function refreshOnboardingPermStatus() {
  if (!invoke) return;
  let granted = false;
  try {
    granted = await invoke("check_accessibility");
  } catch (error) {
    console.error("Failed to check accessibility permission:", error);
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
    alert("Failed to start recording: " + error);
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
    alert("No events recorded yet");
    return;
  }

  try {
    const maxAttempts = parseInt(prompt("Max retry attempts (default 3):", "3") || "3");
    const backoffMs = parseInt(prompt("Backoff ms (default 500):", "500") || "500");
    const backoffMult = parseFloat(prompt("Backoff multiplier (default 2.0):", "2.0") || "2.0");

    isPlaying = true;
    updateRecordingUI();
    await invoke("replay_with_reliability", {
      events: recordedEvents,
      max_attempts: maxAttempts,
      backoff_ms: backoffMs,
      backoff_multiplier: backoffMult,
    });
    isPlaying = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to replay with reliability:", error);
    isPlaying = false;
    updateRecordingUI();
    alert("Replay failed: " + error);
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
  if (!invoke) return;
  try {
    const x = window.screen.width / 2;
    const y = window.screen.height / 2;
    const element = await invoke("inspect_element", { x, y });
    if (element) {
      alert(`Element found:\nRole: ${element.role}\nName: ${element.name}\nApp: ${element.app}`);
    } else {
      alert("No element found at cursor position");
    }
  } catch (error) {
    console.error("Failed to inspect element:", error);
  }
}

// ===== Workflow management =====

async function saveWorkflow() {
  if (!invoke) return;
  const name = prompt("Enter workflow name:");
  if (!name) return;

  try {
    const path = await invoke("save_workflow", { name, events: recordedEvents });
    alert(`Workflow saved to: ${path}`);
  } catch (error) {
    console.error("Failed to save workflow:", error);
    alert("Failed to save workflow: " + error);
  }
}

async function loadWorkflow() {
  if (!invoke) return;
  const name = prompt("Enter workflow name to load:");
  if (!name) return;

  try {
    recordedEvents = await invoke("load_workflow", { name });
    updateRecordingUI();
    refreshTimeline();
    alert(`Workflow "${name}" loaded with ${recordedEvents.length} events`);
  } catch (error) {
    console.error("Failed to load workflow:", error);
    alert("Failed to load workflow: " + error);
  }
}

// ===== AI-powered workflow functions =====

async function analyzeWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }

  try {
    const name = prompt("Enter workflow name for analysis:", "MyWorkflow") || "MyWorkflow";
    const analysis = await invoke("analyze_workflow", { name, events: recordedEvents });
    displayAnalysisResults(analysis);
  } catch (error) {
    console.error("Failed to analyze workflow:", error);
    alert("Failed to analyze workflow: " + error);
  }
}

async function optimizeWorkflow() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }

  try {
    const optimized = await invoke("optimize_workflow", { events: recordedEvents });
    const originalCount = recordedEvents.length;
    recordedEvents = optimized;
    updateRecordingUI();
    refreshTimeline();
    alert(`Optimized workflow: ${originalCount} events → ${optimized.length} events`);
  } catch (error) {
    console.error("Failed to optimize workflow:", error);
    alert("Failed to optimize workflow: " + error);
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
  if (!invoke) return prompt("Enter workflow name:");
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }

  try {
    const suggestion = await invoke("suggest_workflow_name", { events: recordedEvents });
    return prompt("Suggested workflow name:", suggestion) || suggestion;
  } catch (error) {
    console.error("Failed to suggest name:", error);
    return prompt("Enter workflow name:");
  }
}

async function saveWorkflowWithMetadata() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }

  try {
    const name = await suggestWorkflowName();
    if (!name) return;

    const description = prompt("Workflow description:", "") || "";
    const tagsInput = prompt("Tags (comma-separated):", "") || "";
    const tags = tagsInput.split(",").map((t) => t.trim()).filter((t) => t);

    const path = await invoke("save_workflow_with_metadata", {
      name,
      events: recordedEvents,
      description,
      tags,
    });
    alert(`Workflow saved to: ${path}`);
  } catch (error) {
    console.error("Failed to save workflow:", error);
    alert("Failed to save workflow: " + error);
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
    alert("Could not load settings.");
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
    alert(`Could not save settings: ${error}`);
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

// ===== Cloud sync =====

let cloudSyncState = {
  isAuthenticated: false,
  config: null,
};

async function initCloudSync() {
  if (!invoke) return notAvailable();

  try {
    const apiEndpoint = prompt("API Endpoint:", "https://api.ghost.example.com") || "https://api.ghost.example.com";
    const autoSync = confirm("Enable auto-sync? (OK for yes, Cancel for no)");

    await invoke("init_cloud_sync", {
      config: {
        api_endpoint: apiEndpoint,
        auth_token: null,
        auto_sync: autoSync,
        sync_interval_ms: 30000,
      },
    });

    cloudSyncState.config = { apiEndpoint, autoSync };
    alert("Cloud sync initialized!");
  } catch (error) {
    console.error("Failed to init cloud sync:", error);
    alert("Failed to initialize cloud sync: " + error);
  }
}

async function cloudLogin() {
  if (!invoke) return;

  try {
    const token = prompt("Enter your auth token:") || "";
    if (!token) return;

    const success = await invoke("cloud_authenticate", { token });
    if (success) {
      cloudSyncState.isAuthenticated = true;
      alert("Authenticated successfully!");
    }
  } catch (error) {
    console.error("Cloud auth failed:", error);
    alert("Authentication failed: " + error);
  }
}

async function syncToCloud() {
  if (!invoke) return;
  if (!cloudSyncState.isAuthenticated) {
    alert("Please authenticate first");
    return;
  }

  try {
    const synced = await invoke("cloud_sync_workflows", { events: recordedEvents });
    alert(`Synced ${synced.length} workflows to cloud`);
  } catch (error) {
    console.error("Sync failed:", error);
    alert("Sync failed: " + error);
  }
}

async function createWorkspace() {
  if (!invoke) return;
  if (!cloudSyncState.isAuthenticated) {
    alert("Please authenticate first");
    return;
  }

  try {
    const name = prompt("Workspace name:") || "";
    if (!name) return;

    const workspace = await invoke("create_workspace", {
      name,
      owner_id: "current_user",
    });

    alert(`Created workspace: ${workspace.name}`);
  } catch (error) {
    console.error("Create workspace failed:", error);
    alert("Failed to create workspace: " + error);
  }
}

async function viewAuditLogs() {
  if (!invoke) return;
  if (!cloudSyncState.isAuthenticated) {
    alert("Please authenticate first");
    return;
  }

  try {
    const limit = prompt("Number of logs to retrieve:", "50") || "50";
    const logs = await invoke("get_audit_logs", { limit: parseInt(limit) });
    displayAuditLogs(logs);
  } catch (error) {
    console.error("Failed to get audit logs:", error);
    alert("Failed to get audit logs: " + error);
  }
}

function displayAuditLogs(logs) {
  const modal = document.getElementById("audit-modal");
  if (!modal) return;

  const content = modal.querySelector(".modal-content");
  if (!content) return;

  content.innerHTML = `
    <h3>Audit Logs</h3>
    <table style="width: 100%; border-collapse: collapse; font-size: 0.85rem;">
      <thead>
        <tr style="border-bottom: 1px solid #374151;">
          <th style="text-align: left; padding: 8px;">Timestamp</th>
          <th style="text-align: left; padding: 8px;">User</th>
          <th style="text-align: left; padding: 8px;">Action</th>
          <th style="text-align: left; padding: 8px;">Details</th>
        </tr>
      </thead>
      <tbody>
        ${logs.map((log) => `
          <tr style="border-bottom: 1px solid #374151;">
            <td style="padding: 8px;">${new Date(log.timestamp * 1000).toLocaleString()}</td>
            <td style="padding: 8px;">${log.user_id}</td>
            <td style="padding: 8px;">${log.action}</td>
            <td style="padding: 8px;">${log.details}</td>
          </tr>
        `).join("")}
      </tbody>
    </table>
    <button data-close-modal="audit-modal" style="margin-top: 16px;">Close</button>
  `;

  modal.style.display = "flex";
}

// ===== Event timeline =====

function addEventToTimeline(event) {
  const timelineEl = document.getElementById("events-timeline");
  if (!timelineEl) return;

  const empty = timelineEl.querySelector(".events-timeline__empty");
  if (empty) empty.remove();

  const item = document.createElement("div");
  item.className = "timeline-item";

  let description = "";
  if (event.type) {
    switch (event.type) {
      case "MouseClick":
        description = `Click at (${event.x}, ${event.y}) - Button ${event.button}`;
        if (event.semantic_tag) {
          description += ` [AI: ${event.semantic_tag.action} on ${event.semantic_tag.target}]`;
        }
        break;
      case "Key":
        description = `Key ${event.action === "Down" ? "Down" : "Up"}: ${event.chars || "Code " + event.code}`;
        break;
      case "Scroll":
        description = `Scroll: dx=${event.dx}, dy=${event.dy}`;
        break;
      case "Delay":
        description = `Delay: ${event.ms}ms`;
        break;
      case "Wait":
        description = `Wait: ${getConditionDescription(event.condition)}`;
        break;
      case "VisualCheck":
        description = `Visual Check: threshold=${event.threshold}`;
        break;
      case "Variable":
        description = `Variable: ${event.name} = ${event.value_template}`;
        break;
      default:
        description = JSON.stringify(event);
    }
  } else {
    const eventType = Object.keys(event)[0];
    switch (eventType) {
      case "MouseClick":
        description = `Click at (${event.x}, ${event.y}) - Button ${event.button}`;
        break;
      case "Key":
        description = `Key ${event.action === "Down" ? "Down" : "Up"}: Code ${event.code}`;
        break;
      case "Scroll":
        description = `Scroll: dx=${event.dx}, dy=${event.dy}`;
        break;
      case "Delay":
        description = `Delay: ${event.ms}ms`;
        break;
      default:
        description = JSON.stringify(event);
    }
  }

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
    alert("Failed to start observer: " + error);
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
    alert("No events recorded to observe");
    return;
  }

  try {
    const appName = prompt("Which app are you using?", "Unknown App") || "Unknown";
    const patternsFound = await invoke("observe_events", { events: recordedEvents, app_name: appName });
    showNotification(`Found ${patternsFound} learned patterns from <strong>${appName}</strong>!`);

    const suggestions = await invoke("get_proactive_suggestions");
    if (suggestions.length > 0) displaySuggestions(suggestions);
  } catch (error) {
    console.error("Failed to observe events:", error);
    alert("Failed to observe: " + error);
  }
}

async function generateGeekInsights() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }

  try {
    const appName = prompt("Which app are you analyzing?", "Unknown App") || "Unknown";
    const insights = await invoke("generate_geek_insights", { events: recordedEvents, app_name: appName });
    displayGeekInsights(insights, appName);
  } catch (error) {
    console.error("Failed to generate geek insights:", error);
    alert("Failed to generate insights: " + error);
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
    alert(`Workflow "${name}" created!`);
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
    alert("No events recorded yet");
    return;
  }

  try {
    const appName = prompt("App name for baseline:", "default_app");
    if (appName) await invoke("capture_baseline_screenshot", { name: appName });

    const visualChecks = [
      { event_index: recordedEvents.length - 1, name: "Final State", baseline_screenshot_path: appName ? `${appName}.png` : null, threshold: 0.95 },
    ];

    const success = await invoke("replay_with_visual_check", { events: recordedEvents, visual_checks: visualChecks });
    alert(success ? "Replay completed with visual check!" : "Replay was cancelled");
  } catch (error) {
    console.error("Failed to replay with visual check:", error);
    alert("Replay failed: " + error);
  }
}

async function captureBaseline() {
  if (!invoke) return;

  const name = prompt("Baseline name:");
  if (!name) return;

  try {
    const path = await invoke("capture_baseline_screenshot", { name });
    alert(`Baseline captured: ${path}`);
  } catch (error) {
    console.error("Failed to capture baseline:", error);
    alert("Capture failed: " + error);
  }
}

// ===== Data sources =====

async function createDataSource() {
  if (!invoke) return;

  const name = prompt("Data source name:");
  if (!name) return;

  const type = prompt("Data source type (csv/json/environment):", "environment") || "environment";
  let path = null;
  if (type === "csv" || type === "json") path = prompt("Path to data file:");

  try {
    const sourcePath = await invoke("create_data_source", { name, source_type: type, path });
    alert(`Data source created: ${sourcePath}`);
  } catch (error) {
    console.error("Failed to create data source:", error);
    alert("Create failed: " + error);
  }
}

async function loadVariablesFromSource() {
  if (!invoke) return;

  const name = prompt("Data source name:");
  if (!name) return;

  try {
    const variables = await invoke("load_variables", { data_source_name: name });
    alert(`Loaded ${Object.keys(variables).length} variables`);
    console.log("Variables:", variables);
  } catch (error) {
    console.error("Failed to load variables:", error);
    alert("Load failed: " + error);
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

  bind("initCloudBtn", initCloudSync);
  bind("cloudLoginBtn", cloudLogin);
  bind("cloudSyncBtn", syncToCloud);
  bind("newWorkspaceBtn", createWorkspace);
  bind("auditLogsBtn", viewAuditLogs);

  bind("visualCheckBtn", replayWithVisualCheck);
  bind("captureBaselineBtn", captureBaseline);
  bind("newDataSourceBtn", createDataSource);
  bind("loadVariablesBtn", loadVariablesFromSource);

  bind("perm-grant", requestAccessibility);
  bind("settingsBtn", openSettings);

  // Onboarding navigation
  bind("onboardingStart", () => showOnboardingStep(1));
  bind("onboardingBack", () => showOnboardingStep(0));
  bind("onboardingGrant", onboardingGrant);
  bind("onboardingPermNext", () => showOnboardingStep(2));
  bind("onboardingBack2", () => showOnboardingStep(1));
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
  maybeShowOnboarding();
  syncSpeedFromConfig();
});
