import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const sentryMock = vi.hoisted(() => ({
  init: vi.fn(),
  captureException: vi.fn(),
}));
vi.mock("@sentry/node", () => sentryMock);

const { initSentry, isSentryEnabled, captureException, resetSentryForTests } = await import(
  "./sentry.js"
);

describe("initSentry", () => {
  const originalDsn = process.env.SENTRY_DSN;

  beforeEach(() => {
    resetSentryForTests();
    sentryMock.init.mockClear();
    sentryMock.captureException.mockClear();
  });

  afterEach(() => {
    if (originalDsn === undefined) delete process.env.SENTRY_DSN;
    else process.env.SENTRY_DSN = originalDsn;
  });

  it("is a no-op when SENTRY_DSN is unset — the expected state with no account configured", () => {
    delete process.env.SENTRY_DSN;
    initSentry("worker");
    expect(sentryMock.init).not.toHaveBeenCalled();
    expect(isSentryEnabled()).toBe(false);
  });

  it("initializes with the DSN and a service tag when configured", () => {
    process.env.SENTRY_DSN = "https://example@o0.ingest.sentry.io/1";
    initSentry("worker");
    expect(sentryMock.init).toHaveBeenCalledTimes(1);
    const [options] = sentryMock.init.mock.calls[0] as [Record<string, unknown>];
    expect(options.dsn).toBe("https://example@o0.ingest.sentry.io/1");
    expect(options.initialScope).toEqual({ tags: { service: "worker" } });
    expect(isSentryEnabled()).toBe(true);
  });

  it("is idempotent — a second call does not re-initialize", () => {
    process.env.SENTRY_DSN = "https://example@o0.ingest.sentry.io/1";
    initSentry("worker");
    initSentry("worker");
    expect(sentryMock.init).toHaveBeenCalledTimes(1);
  });
});

describe("captureException", () => {
  beforeEach(() => {
    resetSentryForTests();
    sentryMock.init.mockClear();
    sentryMock.captureException.mockClear();
  });

  it("no-ops when Sentry was never initialized, rather than throwing", () => {
    expect(() => captureException(new Error("boom"))).not.toThrow();
    expect(sentryMock.captureException).not.toHaveBeenCalled();
  });

  it("forwards to Sentry with context as `extra` once initialized", () => {
    process.env.SENTRY_DSN = "https://example@o0.ingest.sentry.io/1";
    initSentry("worker");
    const err = new Error("boom");
    captureException(err, { runId: "r1" });
    expect(sentryMock.captureException).toHaveBeenCalledWith(err, { extra: { runId: "r1" } });
    delete process.env.SENTRY_DSN;
  });
});
