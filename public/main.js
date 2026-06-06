const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

// AI Parrot proactive notifications
const parrotPatterns = [
  "Hey, I noticed you copying that pattern...",
  "That workflow looks repeatable!",
  "Want me to memorize this sequence?",
  "I can do this faster for you next time",
  "This looks like your daily routine, right?"
];

// Interactive proactive suggestion examples
const proactiveExamples = [
  { icon: "🤖", text: "Hey! I noticed you copy-pasting between apps...", action: "Automate this workflow" },
  { icon: "🦜", text: "That looks like your morning routine...", action: "Save as 'Morning Setup'" },
  { icon: "⚡", text: "Geek Mode: 12ms avg delay, bottleneck at step 2", action: "Show insights" },
  { icon: "🤖", text: "I've seen this 47 times today...", action: "Create automation" }
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

// Make parrot interactive - cycle through examples on click
function initInteractiveParrot() {
  const parrotContainer = document.querySelector('.ai-parrot-container');
  if (!parrotContainer) return;
  
  let exampleIndex = 0;
  
  parrotContainer.addEventListener('click', () => {
    const msgEl = document.getElementById("parrotMessage");
    if (!msgEl) return;
    
    const example = proactiveExamples[exampleIndex];
    msgEl.innerHTML = `${example.icon} ${example.text}<br><span style="color: var(--success); font-size: 0.9em; margin-left: 20px; cursor: pointer;">→ ${example.action}</span>`;
    
    exampleIndex = (exampleIndex + 1) % proactiveExamples.length;
    
    // Reset typing animation after showing example
    setTimeout(() => {
      parrotCharIndex = 0;
      typeParrotMessage();
    }, 3000);
  });
  
  // Add hover tooltip
  parrotContainer.title = "Click me! I'm your AI helper 👋";
}

window.addEventListener("DOMContentLoaded", () => {
  revealOnScroll();
  setTimeout(typeParrotMessage, 1000);
  initInteractiveParrot();
});

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
