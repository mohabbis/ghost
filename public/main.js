const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

// AI Parrot proactive notifications
const parrotPatterns = [
  "Hey, I noticed you...",
  "...copy that pattern often",
  "...repeat that workflow",
  "...could automate this",
  "...do this daily, right?"
];

let parrotIndex = 0;
let parrotCharIndex = 0;
const parrotSpeed = 100;

function typeParrotMessage() {
  const msgEl = document.getElementById("parrotMessage");
  if (!msgEl) return;
  
  const currentText = parrotPatterns[parrotIndex];
  const visibleText = currentText.slice(0, parrotCharIndex);
  
  msgEl.innerHTML = visibleText + '<span class="typing-cursor">|</span>';
  
  parrotCharIndex++;
  
  if (parrotCharIndex <= currentText.length) {
    setTimeout(typeParrotMessage, parrotSpeed);
  } else {
    // Text complete, wait then start next pattern
    setTimeout(() => {
      parrotIndex = (parrotIndex + 1) % parrotPatterns.length;
      parrotCharIndex = 0;
      setTimeout(typeParrotMessage, 500);
    }, 2000);
  }
}

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

// Recording state
let isRecording = false;
let recordedEvents = [];

// Listen for ghost events from the backend
if (window.__TAURI__?.event) {
  window.__TAURI__.event.listen("ghost:event", (event) => {
    console.log("Ghost event captured:", event.payload);
    if (isRecording) {
      recordedEvents.push(event.payload);
      updateRecordingUI();
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
    updateRecordingUI();
  } catch (error) {
    console.error("Failed to start recording:", error);
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
    await invoke("replay_workflow", { events: recordedEvents });
  } catch (error) {
    console.error("Failed to replay workflow:", error);
  }
}

function updateRecordingUI() {
  const statusEl = document.querySelector(".recording-status");
  const recordBtn = document.getElementById("recordBtn");
  const stopBtn = document.getElementById("stopBtn");
  const replayBtn = document.getElementById("replayBtn");
  
  if (statusEl) {
    if (isRecording) {
      statusEl.innerHTML = '<span class="pulse" aria-hidden="true"></span> Recording workflow...';
      statusEl.style.color = "#ef4444";
    } else {
      statusEl.innerHTML = '<span class="pulse" aria-hidden="true" style="display:none"></span> Ready to record';
      statusEl.style.color = "#22c55e";
    }
  }
  
  // Update button states
  if (recordBtn) recordBtn.disabled = isRecording;
  if (stopBtn) stopBtn.disabled = !isRecording;
  if (replayBtn) replayBtn.disabled = isRecording || recordedEvents.length === 0;
}

// Initialize UI state
updateRecordingUI();

// Start parrot typing animation
document.addEventListener("DOMContentLoaded", () => {
  setTimeout(typeParrotMessage, 1000);
});

console.log("Ghost - AI Parrot Helper initialized");
