import { invoke } from "@tauri-apps/api/core";

const recordBtn = document.getElementById("recordBtn") as HTMLButtonElement;
const stopBtn = document.getElementById("stopBtn") as HTMLButtonElement;
const replayBtn = document.getElementById("replayBtn") as HTMLButtonElement;
const statusEl = document.getElementById("status") as HTMLElement;

let isRecording = false;
let capturedClick: { x: number; y: number; title: string; role: string } | null = null;

// Listen for click events from Rust
window.__TAURI__.event.listen("ghost:click-captured", (event) => {
  const payload = event.payload as { x: number; y: number; title: string; role: string };
  capturedClick = payload;
  statusEl.textContent = `Captured: "${payload.title}" (${payload.role}) at (${payload.x}, ${payload.y})`;
  replayBtn.disabled = false;
});

recordBtn.addEventListener("click", async () => {
  try {
    const hasPermission = await invoke<boolean>("check_accessibility");
    if (!hasPermission) {
      await invoke("request_accessibility");
      statusEl.textContent = "Please grant accessibility permissions, then restart the app.";
      return;
    }
    
    await invoke("start_recording");
    isRecording = true;
    statusEl.textContent = "Recording... Click anywhere on screen";
    recordBtn.disabled = true;
    stopBtn.disabled = false;
    replayBtn.disabled = true;
  } catch (e) {
    statusEl.textContent = `Error starting recording: ${e}`;
  }
});

stopBtn.addEventListener("click", async () => {
  try {
    await invoke("stop_recording");
    isRecording = false;
    statusEl.textContent = "Stopped. Ready to replay.";
    recordBtn.disabled = false;
    stopBtn.disabled = true;
  } catch (e) {
    statusEl.textContent = `Error stopping recording: ${e}`;
  }
});

replayBtn.addEventListener("click", async () => {
  if (!capturedClick) return;
  try {
    statusEl.textContent = "Replaying click...";
    await invoke("replay_click", { x: capturedClick.x, y: capturedClick.y });
    statusEl.textContent = "Click replayed!";
  } catch (e) {
    statusEl.textContent = `Error replaying click: ${e}`;
  }
});

// Initialize button states
stopBtn.disabled = true;
replayBtn.disabled = true;
