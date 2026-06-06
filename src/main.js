const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

function revealOnScroll() {
  if (prefersReducedMotion.matches || !("IntersectionObserver" in window)) return;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      });
    },
    { threshold: 0.16 },
  );

  document.querySelectorAll(".section, .hero-card, .feature-card").forEach((element) => {
    element.classList.add("reveal");
    observer.observe(element);
  });
}

window.addEventListener("DOMContentLoaded", revealOnScroll);

// Tauri IPC integration for Ghost automation
const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};

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

async function startRecording() {
  if (!invoke) {
    alert("Tauri not available - running in static mode");
    return;
  }
  
  try {
    await invoke("start_recording");
    isRecording = true;
    recordedEvents = [];
    // Clear timeline
    const timelineEl = document.querySelector(".events-timeline");
    if (timelineEl) timelineEl.innerHTML = "";
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to start recording:", error);
    alert("Failed to start recording: " + error);
  }
}

async function stopRecording() {
  if (!invoke) {
    alert("Tauri not available - running in static mode");
    return;
  }
  
  try {
    await invoke("stop_recording");
    isRecording = false;
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to stop recording:", error);
  }
}

async function replayWorkflow() {
  if (!invoke) {
    alert("Tauri not available - running in static mode");
    return;
  }
  
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
  if (!invoke) {
    alert("Tauri not available - running in static mode");
    return;
  }
  
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
      backoff_multiplier: backoffMult
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
    // Refresh timeline
    const timelineEl = document.querySelector(".events-timeline");
    if (timelineEl) {
      timelineEl.innerHTML = "";
      recordedEvents.forEach(event => addEventToTimeline(event));
    }
    alert(`Workflow "${name}" loaded with ${recordedEvents.length} events`);
  } catch (error) {
    console.error("Failed to load workflow:", error);
    alert("Failed to load workflow: " + error);
  }
}

async function inspectElementAtCursor() {
  if (!invoke) return;
  // This would need mouse tracking - simplified version
  try {
    // For demo, use center of screen
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

// ===== AI-Powered Workflow Functions =====

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
  const timelineEl = document.querySelector(".events-timeline");
  if (timelineEl) {
    timelineEl.innerHTML = "";
    recordedEvents.forEach(event => addEventToTimeline(event));
  }
}

async function suggestWorkflowName() {
  if (!invoke) return;
  if (recordedEvents.length === 0) {
    alert("No events recorded yet");
    return;
  }
  
  try {
    const suggestion = await invoke("suggest_workflow_name", { events: recordedEvents });
    const name = prompt("Suggested workflow name:", suggestion) || suggestion;
    return name;
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
    const tags = tagsInput.split(",").map(t => t.trim()).filter(t => t);
    
    const path = await invoke("save_workflow_with_metadata", {
      name,
      events: recordedEvents,
      description,
      tags
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
      ${analysis.patterns.map(p => `<li>${p.description} (confidence: ${(p.confidence * 100).toFixed(1)}%)</li>`).join("")}
    </ul>
    ` : ""}
    
    ${analysis.suggested_optimizations.length > 0 ? `
    <h4>Suggested Optimizations</h4>
    <ul>
      ${analysis.suggested_optimizations.map(o => `<li>${o.description}</li>`).join("")}
    </ul>
    ` : ""}
    
    <button onclick="closeModal('analysis-modal')">Close</button>
  `;
  
  modal.style.display = "block";
}

function closeModal(modalId = "analysis-modal") {
  const modal = document.getElementById(modalId);
  if (modal) {
    modal.style.display = "none";
  }
}

// ===== Cloud Sync Functions =====

let cloudSyncState = {
  isAuthenticated: false,
  config: null
};

async function initCloudSync() {
  if (!invoke) {
    alert("Tauri not available - running in static mode");
    return;
  }
  
  try {
    const apiEndpoint = prompt("API Endpoint:", "https://api.ghost.example.com") || "https://api.ghost.example.com";
    const autoSync = confirm("Enable auto-sync? (OK for yes, Cancel for no)");
    
    await invoke("init_cloud_sync", {
      config: {
        api_endpoint: apiEndpoint,
        auth_token: null,
        auto_sync: autoSync,
        sync_interval_ms: 30000
      }
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
      owner_id: "current_user" // In real app, get from auth
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
        ${logs.map(log => `
          <tr style="border-bottom: 1px solid #374151;">
            <td style="padding: 8px;">${new Date(log.timestamp * 1000).toLocaleString()}</td>
            <td style="padding: 8px;">${log.user_id}</td>
            <td style="padding: 8px;">${log.action}</td>
            <td style="padding: 8px;">${log.details}</td>
          </tr>
        `).join("")}
      </tbody>
    </table>
    <button onclick="closeModal('audit-modal')" style="margin-top: 16px;">Close</button>
  `;
  
  modal.style.display = "block";
}

function addEventToTimeline(event) {
  const timelineEl = document.querySelector(".events-timeline");
  if (!timelineEl) return;
  
  const item = document.createElement("div");
  item.className = "timeline-item";
  
  let description = "";
  switch (event.type || Object.keys(event)[0]) {
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
  
  item.textContent = description;
  timelineEl.appendChild(item);
  timelineEl.scrollTop = timelineEl.scrollHeight;
}

function updateRecordingUI() {
  const statusEl = document.querySelector(".recording-status");
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
  
  // Update button states
  if (recordBtn) recordBtn.disabled = isRecording || isPlaying;
  if (stopBtn) stopBtn.disabled = !isRecording;
  if (replayBtn) replayBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  if (replayReliableBtn) replayReliableBtn.disabled = isRecording || isPlaying || recordedEvents.length === 0;
  if (cancelBtn) cancelBtn.disabled = !isPlaying;
  if (pauseBtn) pauseBtn.disabled = !isPlaying || isPaused;
  if (resumeBtn) resumeBtn.disabled = !isPlaying || !isPaused;
}

// Initialize UI state
updateRecordingUI();
