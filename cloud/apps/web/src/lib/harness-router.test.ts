import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HarnessRouterError, startResponse, uploadFile } from "@/lib/harness-router";

/**
 * The stream is Server-Sent Events in the OpenAI Responses style: `data:`-only
 * frames, no `event:` lines, dispatch on `data.type`. A parser that dispatches
 * on the SSE event name instead sees every frame as `message` and matches
 * nothing — this pins the two things that would silently reintroduce that
 * bug: unknown `type` values must not error, and `response.created`'s
 * recovery identifiers live nested at `response.id` /
 * `response.metadata.session_id`, not at the frame's top level.
 */

function sseStream(frames: unknown[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const frame of frames) {
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(frame)}\n\n`));
      }
      controller.close();
    },
  });
}

describe("startResponse", () => {
  beforeEach(() => {
    process.env.HR_API_KEY = "sk-hr-test";
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("ignores unknown frame types and returns ids from the nested response.created payload", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) =>
      new Response(
        sseStream([
          { type: "response.in_progress", sequence_number: 0 },
          { type: "some.future.frame.type", whatever: true },
          {
            type: "response.created",
            sequence_number: 1,
            response: { id: "resp_abc", metadata: { session_id: "hsess_xyz" } },
          },
          // Never read: startResponse returns as soon as it has the ids.
          { type: "response.output_text.delta", delta: "should not be read" },
        ]),
        { status: 200 },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await startResponse({
      harnessId: "h1",
      input: "do the thing",
      idempotencyKey: "idem-1",
    });

    expect(result).toEqual({ responseId: "resp_abc", sessionId: "hsess_xyz" });

    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("https://api.harnessrouter.ai/h1/v1/responses");
    expect((init as RequestInit).method).toBe("POST");
    const headers = (init as RequestInit).headers as Record<string, string>;
    expect(headers.Authorization).toBe("Bearer sk-hr-test");
    expect(headers["Idempotency-Key"]).toBe("idem-1");
    const body = JSON.parse((init as RequestInit).body as string);
    expect(body.stream).toBe(true);
    expect(body.input[0].content[0]).toEqual({ type: "input_text", text: "do the thing" });
  });

  it("throws HarnessRouterError with the response status on a non-2xx", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ detail: "bad key" }), { status: 401 })),
    );

    await expect(
      startResponse({ harnessId: "h1", input: "x", idempotencyKey: "idem-2" }),
    ).rejects.toMatchObject(new HarnessRouterError("bad key", 401));
  });

  it("reads the upload's `id` field, not `file_id`", async () => {
    // Regression: the upload response names this `id`; the *session* files
    // API returns `file_id` for a different id entirely. Reading `file_id`
    // here yielded `undefined`, the `input_file` block was still accepted,
    // and the agent silently ran with no attachment — reporting only that it
    // could not find the file. A wrong key must fail loudly, not silently.
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(
            JSON.stringify({ id: "file_abc", object: "file", filename: "trace.json" }),
            { status: 200 },
          ),
      ),
    );

    await expect(uploadFile(Buffer.from("{}"), "trace.json", "application/json")).resolves.toEqual({
      fileId: "file_abc",
    });
  });

  it("throws rather than attaching an undefined id when the upload returns no id", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ object: "file" }), { status: 200 })),
    );

    await expect(
      uploadFile(Buffer.from("{}"), "trace.json", "application/json"),
    ).rejects.toThrow(/no file id/);
  });

  it("puts the uploaded file id into an input_file block", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) =>
      new Response(
        sseStream([
          {
            type: "response.created",
            response: { id: "resp_1", metadata: { session_id: "hsess_1" } },
          },
        ]),
        { status: 200 },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await startResponse({
      harnessId: "h1",
      input: "convert it",
      idempotencyKey: "idem-4",
      fileIds: ["file_abc"],
    });

    const body = JSON.parse((fetchMock.mock.calls[0]![1] as RequestInit).body as string);
    expect(body.input[0].content).toContainEqual({ type: "input_file", file_id: "file_abc" });
  });

  it("throws if the stream ends without a response.created frame", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(sseStream([{ type: "response.in_progress" }]), { status: 200 })),
    );

    await expect(
      startResponse({ harnessId: "h1", input: "x", idempotencyKey: "idem-3" }),
    ).rejects.toThrow(/before response\.created/);
  });
});
