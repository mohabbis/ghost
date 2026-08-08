"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import type {
  CaptureClientMessage,
  CaptureServerMessage,
} from "@ghost/core/recording/capture-protocol";

/**
 * The live view of a browser running in Ghost's worker, and the input that
 * drives it.
 *
 * The point of this screen is that recording a workflow requires nothing to be
 * installed. The previous two ways in — load an unpacked Chrome extension in
 * developer mode, or produce a trace file and upload it — both ask an
 * operations manager to do something they have no reason to know how to do.
 * This asks them to use a browser.
 *
 * What it renders is a JPEG screencast: frames arrive over a WebSocket and are
 * painted to a canvas, and mouse and keyboard events are sent back. Frames are
 * relayed, never stored — a capture session shows the customer's real inbox and
 * their real ERP, and Ghost has no business keeping a video of that. What *is*
 * kept is the trace the recorder builds inside the page from accessible roles
 * and names, and only if the user presses Save.
 */

interface Props {
  socketUrl: string;
  ticket: string;
  startUrl: string;
  width: number;
  height: number;
}

type Phase = "connecting" | "recording" | "saving" | "done" | "failed";

export function CaptureSession({ socketUrl, ticket, startUrl, width, height }: Props) {
  const router = useRouter();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const socketRef = useRef<WebSocket | null>(null);

  const [phase, setPhase] = useState<Phase>("connecting");
  const [url, setUrl] = useState(startUrl);
  const [address, setAddress] = useState(startUrl);
  const [events, setEvents] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [deadline, setDeadline] = useState<number | null>(null);
  const [remaining, setRemaining] = useState<string>("");

  const send = useCallback((msg: CaptureClientMessage) => {
    const socket = socketRef.current;
    if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(msg));
  }, []);

  // --- socket ------------------------------------------------------------

  useEffect(() => {
    const socket = new WebSocket(socketUrl);
    socketRef.current = socket;

    socket.onopen = () => {
      // The ticket goes in the first message rather than the URL so it never
      // reaches an access log or a `Referer` header.
      socket.send(JSON.stringify({ t: "auth", ticket } satisfies CaptureClientMessage));
    };

    socket.onmessage = (raw) => {
      let msg: CaptureServerMessage;
      try {
        msg = JSON.parse(String(raw.data)) as CaptureServerMessage;
      } catch {
        return;
      }
      switch (msg.t) {
        case "ready":
          setPhase("recording");
          setUrl(msg.url);
          setAddress(msg.url);
          setDeadline(msg.deadline);
          return;
        case "frame":
          paint(canvasRef.current, msg.data);
          return;
        case "url":
          setUrl(msg.url);
          setAddress(msg.url);
          return;
        case "events":
          setEvents(msg.count);
          return;
        case "stopped":
          setPhase("done");
          router.push(`/recordings/${msg.recordingId}`);
          router.refresh();
          return;
        case "canceled":
          setPhase("done");
          router.push("/recordings");
          return;
        case "error":
          setError(msg.message);
          setPhase("failed");
          return;
      }
    };

    socket.onerror = () => {
      setError("Lost the connection to the recording browser.");
      setPhase("failed");
    };
    socket.onclose = () => {
      // Only a surprise if we were still recording. A close after `stopped` or
      // `canceled` is the session ending normally.
      setPhase((p) => (p === "recording" || p === "connecting" ? "failed" : p));
      setError((e) =>
        e ?? "The recording browser disconnected. Nothing was saved — start a new recording.",
      );
    };

    return () => socket.close();
  }, [socketUrl, ticket, router]);

  // --- the session clock -------------------------------------------------

  useEffect(() => {
    if (deadline === null) return;
    const tick = () => {
      const left = Math.max(0, deadline - Date.now());
      const mins = Math.floor(left / 60_000);
      const secs = Math.floor((left % 60_000) / 1000);
      setRemaining(`${mins}:${String(secs).padStart(2, "0")}`);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [deadline]);

  // --- input -------------------------------------------------------------

  // Canvas pixels are not remote-browser pixels: the canvas is laid out
  // responsively and the remote viewport is fixed, so every coordinate is
  // scaled through the element's rendered size. Skipping this is how a click
  // lands next to the button the user aimed at.
  const toRemote = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = e.currentTarget.getBoundingClientRect();
      return {
        x: Math.round(((e.clientX - rect.left) / rect.width) * width),
        y: Math.round(((e.clientY - rect.top) / rect.height) * height),
      };
    },
    [width, height],
  );

  const mouse = useCallback(
    (type: "mousePressed" | "mouseReleased" | "mouseMoved") =>
      (e: React.MouseEvent<HTMLCanvasElement>) => {
        const { x, y } = toRemote(e);
        send({
          t: "mouse",
          type,
          x,
          y,
          button: type === "mouseMoved" ? "none" : "left",
          clickCount: type === "mouseMoved" ? 0 : e.detail || 1,
          modifiers: modifiersOf(e),
        });
      },
    [send, toRemote],
  );

  const onWheel = useCallback(
    (e: React.WheelEvent<HTMLCanvasElement>) => {
      const rect = e.currentTarget.getBoundingClientRect();
      send({
        t: "mouse",
        type: "mouseWheel",
        x: Math.round(((e.clientX - rect.left) / rect.width) * width),
        y: Math.round(((e.clientY - rect.top) / rect.height) * height),
        deltaX: -e.deltaX,
        deltaY: -e.deltaY,
      });
    },
    [send, width, height],
  );

  const onKey = useCallback(
    (type: "keyDown" | "keyUp") => (e: React.KeyboardEvent) => {
      // The remote browser is the one that should react to Tab, Enter and the
      // rest; letting this page handle them would move focus out of the live
      // view mid-recording.
      e.preventDefault();
      const printable = e.key.length === 1 && !e.ctrlKey && !e.metaKey;
      send({
        t: "key",
        // `char` is what actually produces text in CDP; a bare keyDown moves
        // the caret but types nothing.
        type: type === "keyDown" && printable ? "char" : type,
        key: e.key,
        code: e.code,
        text: printable ? e.key : undefined,
        modifiers: modifiersOf(e),
        windowsVirtualKeyCode: virtualKeyCode(e.key),
      });
    },
    [send],
  );

  const onPaste = useCallback(
    (e: React.ClipboardEvent) => {
      e.preventDefault();
      const value = e.clipboardData.getData("text");
      if (value) send({ t: "text", value });
    },
    [send],
  );

  // --- render ------------------------------------------------------------

  const live = phase === "recording";

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="secondary"
          onClick={() => send({ t: "back" })}
          disabled={!live}
          aria-label="Back"
        >
          ←
        </Button>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => send({ t: "forward" })}
          disabled={!live}
          aria-label="Forward"
        >
          →
        </Button>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => send({ t: "reload" })}
          disabled={!live}
          aria-label="Reload"
        >
          ⟳
        </Button>
        <form
          className="flex min-w-0 flex-1 gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            send({ t: "navigate", url: address });
          }}
        >
          <input
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            disabled={!live}
            spellCheck={false}
            aria-label="Address"
            className="h-8 min-w-0 flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 text-sm"
          />
          <Button size="sm" variant="secondary" type="submit" disabled={!live}>
            Go
          </Button>
        </form>
      </div>

      <div className="relative overflow-hidden rounded-lg border border-[var(--color-border)] bg-black">
        <canvas
          ref={canvasRef}
          width={width}
          height={height}
          tabIndex={0}
          onMouseDown={mouse("mousePressed")}
          onMouseUp={mouse("mouseReleased")}
          onMouseMove={mouse("mouseMoved")}
          onWheel={onWheel}
          onKeyDown={onKey("keyDown")}
          onKeyUp={onKey("keyUp")}
          onPaste={onPaste}
          onContextMenu={(e) => e.preventDefault()}
          className="block h-auto w-full cursor-default outline-none"
          aria-label="Recording browser"
        />
        {!live && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/70 p-6 text-center text-sm text-white">
            {phase === "connecting" && "Starting a browser…"}
            {phase === "saving" && "Saving the recording…"}
            {phase === "done" && "Done."}
            {phase === "failed" && (error ?? "This recording session ended.")}
          </div>
        )}
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 text-sm">
        <div className="text-[var(--color-muted)]">
          {/* Counted, not just spun: a recorder that has captured nothing looks
              exactly like one that is working, right up until the review page
              is empty. */}
          <span aria-live="polite">
            {events} action{events === 1 ? "" : "s"} captured
          </span>
          {remaining && live && <span> · {remaining} left</span>}
          <span className="block truncate text-xs">{url}</span>
        </div>
        <div className="flex gap-2">
          <Button
            variant="secondary"
            onClick={() => send({ t: "cancel" })}
            disabled={!live}
          >
            Discard
          </Button>
          <Button
            onClick={() => {
              setPhase("saving");
              send({ t: "stop" });
            }}
            disabled={!live || events === 0}
          >
            Save recording
          </Button>
        </div>
      </div>

      <p className="text-xs text-[var(--color-muted)]">
        Ghost records what you click and type as roles and names — never mouse positions, and
        never the contents of a password, card, or one-time-code field. The live picture above
        is streamed to your screen and not stored. Signing in here does not give Ghost access to
        that account: the browser and its cookies are destroyed when this session ends.
      </p>
    </div>
  );
}

/** Paints one base64 JPEG frame. */
function paint(canvas: HTMLCanvasElement | null, data: string): void {
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const img = new Image();
  img.onload = () => ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  img.src = `data:image/jpeg;base64,${data}`;
}

/** CDP's modifier bitmask: Alt=1, Ctrl=2, Meta=4, Shift=8. */
function modifiersOf(e: { altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
  return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
}

/**
 * Virtual key codes for the keys that need one.
 *
 * Chromium ignores Enter, Tab, Backspace and the arrows without it — they
 * arrive as events the page never acts on, which looks like a recording
 * browser that has frozen. Printable characters need none: they travel as
 * `text` on a `char` event.
 */
const VIRTUAL_KEYS: Record<string, number> = {
  Backspace: 8,
  Tab: 9,
  Enter: 13,
  Escape: 27,
  Space: 32,
  PageUp: 33,
  PageDown: 34,
  End: 35,
  Home: 36,
  ArrowLeft: 37,
  ArrowUp: 38,
  ArrowRight: 39,
  ArrowDown: 40,
  Delete: 46,
};

function virtualKeyCode(key: string): number | undefined {
  return VIRTUAL_KEYS[key];
}
