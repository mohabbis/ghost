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
  if (cancelBtn) cancelBtn.disabled = !isPlaying;
  if (pauseBtn) pauseBtn.disabled = !isPlaying || isPaused;
  if (resumeBtn) resumeBtn.disabled = !isPlaying || !isPaused;
}

// Initialize UI state
updateRecordingUI();
