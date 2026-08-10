import { afterEach, describe, expect, it } from "vitest";
import { checkPublicHttpUrl } from "./public-url.js";

describe("checkPublicHttpUrl", () => {
  const prevAppUrl = process.env.APP_URL;

  afterEach(() => {
    if (prevAppUrl === undefined) delete process.env.APP_URL;
    else process.env.APP_URL = prevAppUrl;
  });

  it.each([
    "https://example.com/path",
    "http://example.com",
    "https://shop.customer.com/orders",
    "https://8.8.8.8/health",
    "http://localhost:3000/fixtures/order",
    "http://127.0.0.1:4123/",
  ])("allows %s", (raw) => {
    delete process.env.APP_URL;
    expect(checkPublicHttpUrl(raw).ok).toBe(true);
  });

  it.each([
    "http://169.254.169.254/latest/meta-data/",
    "http://metadata.google.internal/",
    "ftp://example.com/",
    "file:///etc/passwd",
    "not a url",
    "http://10.0.0.5/secret",
    "http://192.168.1.1/",
    "http://172.16.0.1/",
    "http://printer.local/",
  ])("rejects %s", (raw) => {
    delete process.env.APP_URL;
    expect(checkPublicHttpUrl(raw).ok).toBe(false);
  });

  it("allows a private APP_URL host for self-hosted Ghost", () => {
    process.env.APP_URL = "http://10.0.0.5:3000";
    expect(checkPublicHttpUrl("http://10.0.0.5:3000/fixtures/order").ok).toBe(true);
    expect(checkPublicHttpUrl("http://10.0.0.9/").ok).toBe(false);
  });

  it("never allows cloud metadata even if APP_URL were somehow set to it", () => {
    process.env.APP_URL = "http://169.254.169.254";
    expect(checkPublicHttpUrl("http://169.254.169.254/latest/meta-data/").ok).toBe(false);
  });
});
