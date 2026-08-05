/**
 * Ghost recorder — popup.
 *
 * Deliberately thin: start, stop, and where to send it. The review step is in
 * Ghost, not here. An extension that let you edit steps would be a second
 * editor to keep in sync with the real one, and the whole point of the trust
 * pipeline is that a proposal is reviewed and published in one place.
 */

const toggle = document.getElementById("toggle");
const status = document.getElementById("status");
const urlInput = document.getElementById("url");
const tokenInput = document.getElementById("token");
const settings = document.getElementById("settings");

let recording = false;

function say(html) {
  status.innerHTML = html;
}

async function refresh() {
  const state = await chrome.storage.local.get(["ghostUrl", "token"]);
  urlInput.value = state.ghostUrl || "";
  tokenInput.value = state.token || "";

  chrome.runtime.sendMessage({ kind: "ghost-status" }, (s) => {
    recording = Boolean(s?.recording);
    toggle.textContent = recording ? "Stop and send to Ghost" : "Start recording";
    toggle.classList.toggle("rec", recording);
    if (recording) {
      say(`Recording — ${s.count} event${s.count === 1 ? "" : "s"} captured.`);
    } else if (!s?.configured) {
      say("Set your Ghost URL and API token below to begin.");
      settings.open = true;
    } else {
      say("Ready. Recording captures clicks, typing and navigation — never passwords.");
    }
  });
}

const persist = () =>
  chrome.storage.local.set({
    ghostUrl: urlInput.value.trim(),
    token: tokenInput.value.trim(),
  });

urlInput.addEventListener("change", persist);
tokenInput.addEventListener("change", persist);

toggle.addEventListener("click", async () => {
  await persist();
  toggle.disabled = true;

  if (!recording) {
    chrome.runtime.sendMessage({ kind: "ghost-start" }, async () => {
      // The content script is injected at document_idle, so a tab already open
      // before the extension was installed has no recorder in it yet.
      const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (tab?.id) {
        await chrome.scripting
          .executeScript({ target: { tabId: tab.id }, files: ["src/content.js"] })
          .catch(() => undefined);
      }
      toggle.disabled = false;
      refresh();
    });
    return;
  }

  say("Sending to Ghost…");
  chrome.runtime.sendMessage({ kind: "ghost-stop" }, (result) => {
    toggle.disabled = false;
    if (result?.ok) {
      say(
        `Sent. ${result.stepCount} step${result.stepCount === 1 ? "" : "s"} proposed — ` +
          `<a href="${result.reviewUrl}" target="_blank" rel="noreferrer">review in Ghost</a>.`,
      );
    } else {
      say(`Could not send: ${result?.error || "unknown error"}. Your recording was kept.`);
    }
    refresh();
  });
});

refresh();
