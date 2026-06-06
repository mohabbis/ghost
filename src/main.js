// Ghost frontend: drives the macOS observe/replay commands exposed by the Rust
// backend (see src-tauri/src/commands.rs). withGlobalTauri is on, so the API
// lives on window.__TAURI__.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Recorded clicks: [{ x, y }]. This is the "macro" we replay.
const steps = [];
let recording = false;
let unlistenClick = null;

const els = {};
const REPLAY_DELAY_MS = 600; // pause between synthesized clicks on replay

function $(id) {
  return document.getElementById(id);
}

function setStatus(label, mode) {
  els.statusLabel.textContent = label;
  els.statusLabel.dataset.mode = mode || "idle";
}

function renderSteps() {
  els.stepCount.textContent = `${steps.length} step${steps.length === 1 ? "" : "s"}`;
  els.steps.innerHTML = "";
  steps.forEach((s, i) => {
    const li = document.createElement("li");
    li.className = "steps__item";
    li.innerHTML = `<span class="steps__n">${i + 1}</span> click at (${Math.round(s.x)}, ${Math.round(s.y)})`;
    els.steps.appendChild(li);
  });
  const hasSteps = steps.length > 0;
  els.replayBtn.disabled = !hasSteps || recording;
  els.clearBtn.disabled = !hasSteps || recording;
}

// Permission gating: the backend AX/event-tap calls are useless until macOS
// grants Accessibility, so reflect that state in the UI before enabling Record.
async function refreshPermission() {
  const granted = await invoke("check_accessibility");
  els.permBanner.hidden = granted;
  els.recordBtn.disabled = !granted;
  if (!granted) setStatus("Permission needed", "warn");
  return granted;
}

async function startRecording() {
  if (!(await refreshPermission())) return;

  // ghost:click-captured is emitted from the Rust CGEventTap for every global
  // left mouse-down while recording is active.
  unlistenClick = await listen("ghost:click-captured", (event) => {
    const [x, y] = event.payload;
    steps.push({ x, y });
    renderSteps();
  });

  try {
    await invoke("start_recording");
  } catch (e) {
    if (unlistenClick) unlistenClick();
    unlistenClick = null;
    setStatus(`Error: ${e}`, "warn");
    return;
  }

  recording = true;
  els.recordBtn.classList.add("is-recording");
  // The button's last child is the " Record" text node, after the .dot span.
  els.recordBtn.lastChild.textContent = " Stop";
  setStatus("Recording — go do your thing", "recording");
  renderSteps();
}

async function stopRecording() {
  await invoke("stop_recording");
  if (unlistenClick) unlistenClick();
  unlistenClick = null;
  recording = false;
  els.recordBtn.classList.remove("is-recording");
  els.recordBtn.lastChild.textContent = " Record";
  setStatus(steps.length ? "Ready to replay" : "Idle", "idle");
  renderSteps();
}

async function toggleRecording() {
  if (recording) await stopRecording();
  else await startRecording();
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

async function replay() {
  if (!steps.length || recording) return;
  els.replayBtn.disabled = true;
  els.recordBtn.disabled = true;
  setStatus("Replaying…", "replaying");

  try {
    for (let i = 0; i < steps.length; i++) {
      const { x, y } = steps[i];
      els.statusLabel.textContent = `Replaying ${i + 1}/${steps.length}`;
      await invoke("replay_click", { x, y });
      await wait(REPLAY_DELAY_MS);
    }
    setStatus("Replay complete", "idle");
  } catch (e) {
    setStatus(`Error: ${e}`, "warn");
  } finally {
    els.recordBtn.disabled = false;
    renderSteps();
  }
}

function clearSteps() {
  steps.length = 0;
  setStatus("Idle", "idle");
  renderSteps();
}

window.addEventListener("DOMContentLoaded", async () => {
  Object.assign(els, {
    permBanner: $("perm-banner"),
    permGrant: $("perm-grant"),
    recordBtn: $("record-btn"),
    replayBtn: $("replay-btn"),
    clearBtn: $("clear-btn"),
    statusLabel: $("status-label"),
    stepCount: $("step-count"),
    steps: $("steps"),
  });

  els.recordBtn.addEventListener("click", toggleRecording);
  els.replayBtn.addEventListener("click", replay);
  els.clearBtn.addEventListener("click", clearSteps);
  els.permGrant.addEventListener("click", async () => {
    await invoke("request_accessibility");
    // macOS grants only take effect on the running binary; poll for the change.
    await refreshPermission();
  });

  await refreshPermission();
  renderSteps();
});
