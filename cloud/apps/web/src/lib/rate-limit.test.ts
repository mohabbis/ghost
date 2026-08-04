import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The limiter that stands between a six-digit TOTP code and a scripted
 * exhaustive search. Two properties are worth pinning: it actually refuses
 * past the limit, and it refuses rather than admits when Redis is unreachable.
 *
 * The fail-closed behaviour is the one most likely to be "fixed" by someone
 * who reads a Redis outage as a bug in this file. It is deliberate: failing
 * open means the outage window is exactly when brute force becomes free.
 */

const state = vi.hoisted(() => ({
  counters: new Map<string, number>(),
  throws: false,
}));

vi.mock("ioredis", () => {
  class FakeRedis {
    multi() {
      const ops: (() => void)[] = [];
      let key = "";
      const chain = {
        incr: (k: string) => {
          key = k;
          ops.push(() => state.counters.set(k, (state.counters.get(k) ?? 0) + 1));
          return chain;
        },
        expire: () => chain,
        exec: async () => {
          if (state.throws) throw new Error("redis unreachable");
          ops.forEach((op) => op());
          return [[null, state.counters.get(key) ?? 0]];
        },
      };
      return chain;
    }
    async ttl() {
      return 42;
    }
  }
  return { default: FakeRedis };
});

const { rateLimit, clientKey } = await import("@/lib/rate-limit");

beforeEach(() => {
  state.counters.clear();
  state.throws = false;
  process.env.REDIS_URL ||= "redis://localhost:6379";
});

describe("rateLimit", () => {
  it("admits hits up to the limit and refuses the one after", async () => {
    const opts = { limit: 3, windowSeconds: 60 };
    for (let i = 0; i < 3; i++) {
      expect((await rateLimit("k", opts)).ok).toBe(true);
    }
    const refused = await rateLimit("k", opts);
    expect(refused.ok).toBe(false);
    expect(refused.retryAfter).toBeGreaterThan(0);
  });

  it("counts each key independently", async () => {
    const opts = { limit: 1, windowSeconds: 60 };
    expect((await rateLimit("user:a", opts)).ok).toBe(true);
    expect((await rateLimit("user:b", opts)).ok).toBe(true);
    expect((await rateLimit("user:a", opts)).ok).toBe(false);
  });

  it("fails closed when Redis is unreachable", async () => {
    state.throws = true;
    const result = await rateLimit("k", { limit: 100, windowSeconds: 60 });
    expect(result.ok).toBe(false);
  });
});

describe("clientKey", () => {
  it("takes the first entry of x-forwarded-for", () => {
    const req = new Request("https://ghost.test/", {
      headers: { "x-forwarded-for": "203.0.113.7, 70.41.3.18" },
    });
    expect(clientKey(req)).toBe("203.0.113.7");
  });

  it("degrades to a constant rather than throwing when there is no address", () => {
    expect(clientKey(new Request("https://ghost.test/"))).toBe("unknown");
  });
});
