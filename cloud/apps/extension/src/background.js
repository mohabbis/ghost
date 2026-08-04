/**
 * Ghost recorder — collection and upload.
 *
 * Holds the event buffer for a recording session and, on stop, posts the trace
 * to Ghost. Nothing is uploaded while recording: a partial trace is not a
 * workflow, and streaming would mean a half-recorded session could be reviewed
 * and published.
 *
 * Auth is a revocable bearer credential the user creates in Ghost Settings,
 * not the session cookie. The extension is a different origin, so a
 * `SameSite=Lax` session cookie is not reliably sent on a cross-site POST —
 * and a token can be revoked from Ghost without touching the browser.
 *
 * It uploads to `/api/agent/recordings`, the "propose" side of the trust
 * boundary. The extension cannot publish a workflow or approve anything; it
 * hands over a proposal for a human to review.
 */

const STORAGE = chrome.storage.local;

async function getState() {
  return STORAGE.get(["recording", "events", "ghostUrl", "token", "startedAt"]);
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.kind === "ghost-event") {
    // Fire-and-forget from the page's perspective; the append is async.
    void appendEvent(message.event);
    return false;
  }

  if (message?.kind === "ghost-start") {
    void start().then(sendResponse);
    return true; // async response
  }

  if (message?.kind === "ghost-stop") {
    void stop().then(sendResponse);
    return true;
  }

  if (message?.kind === "ghost-status") {
    void getState().then((s) =>
      sendResponse({
        recording: Boolean(s.recording),
        count: (s.events || []).length,
        configured: Boolean(s.ghostUrl && s.token),
      }),
    );
    return true;
  }

  return false;
});

/** Cap the buffer so a forgotten recording cannot grow without bound. */
const MAX_EVENTS = 5000;

async function appendEvent(event) {
  const { recording, events } = await STORAGE.get(["recording", "events"]);
  if (!recording) return;
  const next = events || [];
  if (next.length >= MAX_EVENTS) return;
  next.push(event);
  await STORAGE.set({ events: next });
  chrome.action.setBadgeText({ text: String(next.length) });
  chrome.action.setBadgeBackgroundColor({ color: "#b45309" });
}

async function start() {
  await STORAGE.set({ recording: true, events: [], startedAt: Date.now() });
  chrome.action.setBadgeText({ text: "0" });
  return { ok: true };
}

async function stop() {
  const state = await getState();
  await STORAGE.set({ recording: false });
  chrome.action.setBadgeText({ text: "" });

  const events = state.events || [];
  if (events.length === 0) {
    return { ok: false, error: "Nothing was recorded." };
  }
  if (!state.ghostUrl || !state.token) {
    return { ok: false, error: "Set your Ghost URL and API token first." };
  }

  const trace = {
    version: 1,
    sessionId: `rec_${state.startedAt || Date.now()}`,
    startUrl: events.find((e) => e.url)?.url,
    capturedAt: Date.now(),
    recorder: "ghost-chrome-extension/0.1.0",
    events,
  };

  try {
    const response = await fetch(`${state.ghostUrl.replace(/\/$/, "")}/api/agent/recordings`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${state.token}`,
      },
      body: JSON.stringify(trace),
    });

    const body = await response.json().catch(() => null);
    if (!response.ok) {
      return { ok: false, error: body?.error || `Ghost returned ${response.status}.` };
    }

    await STORAGE.set({ events: [] });
    return {
      ok: true,
      id: body.id,
      stepCount: body.stepCount,
      reviewUrl: `${state.ghostUrl.replace(/\/$/, "")}/recordings/${body.id}`,
    };
  } catch (err) {
    // The events are deliberately kept so a failed upload can be retried
    // rather than discarding a recording the user cannot easily repeat.
    return { ok: false, error: `Could not reach Ghost: ${err.message}` };
  }
}
