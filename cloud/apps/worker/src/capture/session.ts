import type { BrowserContext, CDPSession, Page } from "playwright";
import { chromium } from "playwright";
import {
  CAPTURE_LIMITS,
  CAPTURE_VIEWPORT,
  type CaptureClientMessage,
  type CaptureServerMessage,
  type CaptureTicketClaims,
} from "@ghost/core/recording/capture";
import { CAPTURE_BINDING, recorderInitScript } from "@ghost/core/recording/recorder";
import { recordingTraceSchema, type RecordingTrace } from "@ghost/core/recording/trace";
import { ingestTrace, type IngestResult } from "@ghost/core/recording/ingest";
import { createLogger } from "@ghost/core/logger";
import { launchOptions } from "../browser/driver.js";

const log = createLogger("capture");

/**
 * Why a session ended. Only `stopped` keeps what was recorded.
 *
 * `disconnected` is separate from `canceled` so the logs say what happened: a
 * user who pressed Cancel made a decision, and a user whose socket dropped did
 * not. They have the same effect on the trace and different meanings.
 */
export type CaptureEndReason = "stopped" | "canceled" | "idle" | "expired" | "disconnected";

/**
 * One remote capture session: a real browser in the worker, driven by a person
 * through their own browser.
 *
 * This is the "record" half of Phase 2, and it is a cloud browser rather than
 * an extension for one reason — an extension is a thing the user has to
 * install, in developer mode, before Ghost can do anything for them at all. A
 * cloud browser is a page they open.
 *
 * ## What crosses which boundary
 *
 * **Coordinates go in; they never come out.** A person moving a mouse produces
 * pixel positions, and forwarding them is the only way to drive a remote
 * browser. But the recorder injected into the page reads the *accessible role
 * and name* of whatever the pointer landed on, off the live element, and that
 * is what the trace carries. Coordinates drive the session and stop at the
 * page; they are not automation identity, and there is no step field that
 * could hold one.
 *
 * **Frames go out; they are never stored.** The live view is a CDP screencast
 * relayed to the socket and dropped. A capture session shows the customer's
 * real systems — their inbox, their ERP — and writing that to the artifact
 * store would create a video of it that nobody asked for. Run screenshots are
 * different: they are evidence for an approval, deliberately retained, and
 * covered by the retention window.
 *
 * **Secrets never enter the trace.** Enforced in the page by the recorder,
 * which is the only place it can be done honestly. A password typed here is
 * dispatched into the remote browser and recorded as `redacted: true` with no
 * value.
 *
 * **No session state is kept.** The browser context is destroyed when the
 * session ends. If the user signed into their bank to demonstrate a workflow,
 * Ghost does not keep those cookies — holding live credentials for a system
 * needs an explicit, scoped grant, and demonstrating a task is not one.
 */
export class CaptureSession {
  private events: unknown[] = [];
  private droppedEvents = 0;
  private closed = false;
  private startedAt = Date.now();
  private lastClientMessageAt = Date.now();
  private cdp: CDPSession | null = null;
  private timers: NodeJS.Timeout[] = [];

  private constructor(
    private readonly context: BrowserContext,
    private readonly page: Page,
    private readonly claims: CaptureTicketClaims,
    private readonly send: (msg: CaptureServerMessage) => void,
    private readonly onClosed: () => void,
  ) {}

  static async open(
    claims: CaptureTicketClaims,
    send: (msg: CaptureServerMessage) => void,
    onClosed: () => void,
  ): Promise<CaptureSession> {
    const browser = await chromium.launch(launchOptions());
    const context = await browser.newContext({ viewport: { ...CAPTURE_VIEWPORT } });

    // Order matters: the binding installs its own init script, so exposing it
    // first means it exists by the time the recorder's script runs. The
    // recorder also queues and retries, because "first" is not a guarantee
    // worth betting a whole recording on.
    const page = await context.newPage();
    const session = new CaptureSession(context, page, claims, send, onClosed);

    await context.exposeBinding(CAPTURE_BINDING, (_source, event: unknown) => {
      session.collect(event);
    });
    await context.addInitScript({ content: recorderInitScript(CAPTURE_BINDING) });

    await session.start(browser);
    return session;
  }

  private async start(browser: { close(): Promise<void> }): Promise<void> {
    this.onBrowserClose = () => browser.close();

    // Report the address bar back so the user can see where they are, and so a
    // redirect they did not initiate is visible rather than silent.
    this.page.on("framenavigated", (frame) => {
      if (frame === this.page.mainFrame()) {
        this.send({ t: "url", url: frame.url() });
      }
    });

    await this.page
      .goto(this.claims.startUrl, { waitUntil: "domcontentloaded", timeout: 30_000 })
      .catch((err: unknown) => {
        // A bad start URL must not kill the session — the user can type
        // another one. Report it and stay open.
        this.send({
          t: "error",
          message: `Could not open ${this.claims.startUrl}: ${describeError(err)}`,
        });
      });

    this.cdp = await this.context.newCDPSession(this.page);
    this.cdp.on("Page.screencastFrame", (frame: { data: string; sessionId: number }) => {
      this.send({ t: "frame", data: frame.data });
      // Acked immediately rather than on the client's ack: an unacked
      // screencast simply stops, and a dropped frame is better than a frozen
      // live view that makes the user think the session died.
      this.cdp?.send("Page.screencastFrameAck", { sessionId: frame.sessionId }).catch(() => undefined);
    });
    await this.cdp.send("Page.startScreencast", {
      format: "jpeg",
      quality: 60,
      maxWidth: CAPTURE_LIMITS.frameWidth,
      maxHeight: CAPTURE_LIMITS.frameHeight,
      everyNthFrame: 1,
    });

    this.send({
      t: "ready",
      url: this.page.url(),
      width: CAPTURE_VIEWPORT.width,
      height: CAPTURE_VIEWPORT.height,
      deadline: this.startedAt + CAPTURE_LIMITS.maxSessionMs,
    });

    // Both ceilings exist so an abandoned tab cannot pin a browser forever;
    // enough abandoned sessions would take the worker down and replay with it.
    this.timers.push(
      setInterval(() => {
        if (Date.now() - this.lastClientMessageAt > CAPTURE_LIMITS.idleTimeoutMs) {
          void this.finish("idle");
        } else if (Date.now() - this.startedAt > CAPTURE_LIMITS.maxSessionMs) {
          void this.finish("expired");
        }
      }, 10_000),
    );
  }

  private onBrowserClose: (() => Promise<void>) | null = null;

  /** An event from the page-side recorder. */
  private collect(event: unknown): void {
    if (this.closed) return;
    if (this.events.length >= CAPTURE_LIMITS.maxEvents) {
      // Counted rather than silently ignored: the reviewer is told the
      // recording is incomplete instead of being handed a truncated workflow
      // that looks whole.
      this.droppedEvents++;
      return;
    }
    this.events.push(event);
    // Cheap progress signal. The user is watching a page, not a log, and needs
    // to know Ghost is actually seeing what they do.
    this.send({ t: "events", count: this.events.length });
  }

  /** A message from the user's browser. */
  async handle(msg: CaptureClientMessage): Promise<void> {
    this.lastClientMessageAt = Date.now();
    if (this.closed) return;

    switch (msg.t) {
      case "mouse":
        await this.cdp?.send("Input.dispatchMouseEvent", {
          type: msg.type,
          x: msg.x,
          y: msg.y,
          button: msg.button ?? "left",
          clickCount: msg.clickCount ?? (msg.type === "mouseMoved" ? 0 : 1),
          deltaX: msg.deltaX ?? 0,
          deltaY: msg.deltaY ?? 0,
          modifiers: msg.modifiers ?? 0,
        });
        return;
      case "key":
        await this.cdp?.send("Input.dispatchKeyEvent", {
          type: msg.type,
          key: msg.key,
          code: msg.code,
          text: msg.text,
          modifiers: msg.modifiers ?? 0,
          windowsVirtualKeyCode: msg.windowsVirtualKeyCode,
        });
        return;
      case "text":
        await this.cdp?.send("Input.insertText", { text: msg.value });
        return;
      case "navigate":
        await this.page
          .goto(normalizeUrl(msg.url), { waitUntil: "domcontentloaded", timeout: 30_000 })
          .catch((err: unknown) =>
            this.send({ t: "error", message: `Could not open that page: ${describeError(err)}` }),
          );
        return;
      case "back":
        await this.page.goBack().catch(() => undefined);
        return;
      case "forward":
        await this.page.goForward().catch(() => undefined);
        return;
      case "reload":
        await this.page.reload().catch(() => undefined);
        return;
      case "stop":
        await this.finish("stopped");
        return;
      case "cancel":
        await this.finish("canceled");
        return;
      case "ack":
        return;
      case "auth":
        // Consumed by the server before this session existed. A second one is
        // not a way to change orgs mid-session.
        return;
    }
  }

  /**
   * End the session.
   *
   * `stopped` saves and compiles; everything else throws the trace away. A
   * session that timed out or was abandoned produced a partial demonstration,
   * and a half-recorded workflow presented as a result is worse than none —
   * the reviewer has no way to tell which half is missing.
   */
  async finish(reason: CaptureEndReason): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    for (const t of this.timers) clearInterval(t);

    try {
      if (reason === "stopped") {
        const result = await this.save();
        this.send({
          t: "stopped",
          recordingId: result.id,
          compileStatus: result.compileStatus,
          stepCount: result.stepCount,
          notes: this.droppedEvents > 0
            ? [
                ...result.notes,
                `This recording hit Ghost's ${CAPTURE_LIMITS.maxEvents}-event ceiling and ` +
                  `${this.droppedEvents} later actions were not captured. Review the end of the ` +
                  "workflow carefully, or record it in smaller pieces.",
              ]
            : result.notes,
        });
      } else if (reason === "canceled" || reason === "disconnected") {
        // Nothing is sent for `disconnected` in practice — the socket that
        // would receive it is what went away — but the trace is dropped for
        // the same reason a cancel drops it: nobody pressed Stop.
        this.send({ t: "canceled" });
      } else {
        this.send({
          t: "error",
          message:
            reason === "idle"
              ? "Capture ended: nothing happened for two minutes, so the browser was released."
              : "Capture ended: sessions are capped at 15 minutes. Record the workflow in smaller pieces.",
        });
      }
    } catch (err) {
      log.error("capture finish failed", { sessionId: this.claims.sessionId, err });
      this.send({ t: "error", message: `Could not save this recording: ${describeError(err)}` });
    } finally {
      await this.dispose();
      this.onClosed();
    }
  }

  private async save(): Promise<IngestResult> {
    const trace: RecordingTrace = recordingTraceSchema.parse({
      version: 1,
      sessionId: this.claims.sessionId,
      startUrl: this.claims.startUrl,
      capturedAt: this.startedAt,
      recorder: "ghost-cloud-browser",
      events: this.events,
    });

    // Through the same path an uploaded trace takes, deliberately: one
    // definition of what a Recording is, one set of limits, one audit shape.
    // `ingestTrace` is also what creates the `Recording` row — nothing exists
    // in the database until this moment, so an abandoned capture leaves no
    // empty recording for someone to find later and have to explain.
    return ingestTrace({
      orgId: this.claims.orgId,
      userId: this.claims.userId,
      filename: `cloud-capture-${this.claims.sessionId}.json`,
      contentType: "application/json",
      buffer: Buffer.from(JSON.stringify(trace), "utf8"),
    });
  }

  private async dispose(): Promise<void> {
    await this.cdp?.send("Page.stopScreencast").catch(() => undefined);
    await this.cdp?.detach().catch(() => undefined);
    // Closing the context discards cookies and storage with it. Nothing the
    // user signed into during the demonstration is kept.
    await this.context.close().catch(() => undefined);
    await this.onBrowserClose?.().catch(() => undefined);
  }
}

function describeError(err: unknown): string {
  return err instanceof Error ? err.message.split("\n")[0]! : String(err);
}

/** Accepts what a person types into an address bar. */
export function normalizeUrl(input: string): string {
  const trimmed = input.trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  return `https://${trimmed}`;
}
