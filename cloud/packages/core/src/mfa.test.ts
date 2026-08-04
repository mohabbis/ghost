import { describe, expect, it } from "vitest";
import {
  base32Decode,
  base32Encode,
  generateRecoveryCodes,
  generateTotpSecret,
  hashRecoveryCode,
  hotp,
  normalizeRecoveryCode,
  totp,
  totpUri,
  verifyTotp,
} from "./mfa.js";

/**
 * The point of these tests is the published vectors.
 *
 * A hand-rolled TOTP that is subtly wrong still *looks* fine — it generates
 * six digits, they change every thirty seconds, and enrolment appears to
 * work — right up until a real authenticator app disagrees with it. Checking
 * against RFC 4226 and RFC 6238's own vectors is what actually establishes
 * interoperability.
 */

// Both RFCs use the ASCII secret "12345678901234567890".
const RFC_SECRET_ASCII = "12345678901234567890";
const RFC_SECRET_B32 = base32Encode(Buffer.from(RFC_SECRET_ASCII, "utf8"));

describe("RFC 4226 HOTP test vectors (Appendix D)", () => {
  const expected = [
    "755224",
    "287082",
    "359152",
    "969429",
    "338314",
    "254676",
    "287922",
    "162583",
    "399871",
    "520489",
  ];
  it.each(expected.map((code, counter) => [counter, code]))(
    "counter %i produces %s",
    (counter, code) => {
      expect(hotp(Buffer.from(RFC_SECRET_ASCII, "utf8"), counter as number)).toBe(code);
    },
  );
});

describe("RFC 6238 TOTP test vectors (Appendix B, SHA-1)", () => {
  // The RFC tabulates 8-digit codes at specific Unix times.
  const vectors: [number, string][] = [
    [59, "94287082"],
    [1111111109, "07081804"],
    [1111111111, "14050471"],
    [1234567890, "89005924"],
    [2000000000, "69279037"],
    [20000000000, "65353130"],
  ];
  it.each(vectors)("t=%i produces %s", (seconds, code) => {
    expect(totp(RFC_SECRET_B32, seconds * 1000, 8)).toBe(code);
  });
});

describe("base32", () => {
  it("round-trips arbitrary bytes", () => {
    for (const len of [1, 2, 5, 10, 20, 33]) {
      const buf = Buffer.from(Array.from({ length: len }, (_, i) => (i * 37) % 256));
      expect(base32Decode(base32Encode(buf))).toEqual(buf);
    }
  });

  it("tolerates the spacing and padding people paste from a screen", () => {
    const secret = generateTotpSecret();
    const messy = `${secret.slice(0, 4)} ${secret.slice(4, 8)}-${secret.slice(8)}==`;
    expect(base32Decode(messy)).toEqual(base32Decode(secret));
  });

  it("rejects characters outside the alphabet", () => {
    expect(() => base32Decode("ABC1")).toThrow(/invalid base32/);
  });
});

describe("verifyTotp", () => {
  const secret = generateTotpSecret();
  const now = 1_700_000_000_000;

  it("accepts the current code", () => {
    expect(verifyTotp(secret, totp(secret, now), { atMs: now })).toBe(true);
  });

  it("accepts one step of drift either way, and rejects two", () => {
    const step = 30_000;
    expect(verifyTotp(secret, totp(secret, now - step), { atMs: now })).toBe(true);
    expect(verifyTotp(secret, totp(secret, now + step), { atMs: now })).toBe(true);
    expect(verifyTotp(secret, totp(secret, now - 2 * step), { atMs: now })).toBe(false);
    expect(verifyTotp(secret, totp(secret, now + 2 * step), { atMs: now })).toBe(false);
  });

  it("rejects malformed input without throwing", () => {
    for (const bad of ["", "abcdef", "12345", "1234567", "12 34 56 78", "٠١٢٣٤٥"]) {
      expect(verifyTotp(secret, bad, { atMs: now })).toBe(false);
    }
  });

  it("rejects a code generated from a different secret", () => {
    expect(verifyTotp(secret, totp(generateTotpSecret(), now), { atMs: now })).toBe(false);
  });
});

describe("totpUri", () => {
  it("carries issuer in both the label and the parameters", () => {
    const uri = totpUri({ secret: "ABCD", account: "person@example.com", issuer: "Ghost" });
    expect(uri.startsWith("otpauth://totp/Ghost:person%40example.com?")).toBe(true);
    const params = new URLSearchParams(uri.split("?")[1]);
    expect(params.get("issuer")).toBe("Ghost");
    expect(params.get("secret")).toBe("ABCD");
    expect(params.get("algorithm")).toBe("SHA1");
    expect(params.get("digits")).toBe("6");
    expect(params.get("period")).toBe("30");
  });
});

describe("recovery codes", () => {
  it("issues distinct, high-entropy codes", () => {
    const codes = generateRecoveryCodes();
    expect(codes).toHaveLength(10);
    expect(new Set(codes).size).toBe(10);
    for (const c of codes) expect(c).toMatch(/^[a-z2-7]{4}(-[a-z2-7]{4}){3}$/);
  });

  it("hashes to a digest that does not contain the code", () => {
    const [code] = generateRecoveryCodes(1);
    expect(hashRecoveryCode(code!)).not.toContain(code!.replace(/-/g, ""));
  });

  it("normalizes the formatting people actually type", () => {
    const [code] = generateRecoveryCodes(1);
    expect(hashRecoveryCode(`  ${code!.toUpperCase()} `)).toBe(hashRecoveryCode(code!));
    expect(normalizeRecoveryCode(" AB-CD ")).toBe("ab-cd");
  });
});
