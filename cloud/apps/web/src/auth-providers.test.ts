import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Guards the shape of the Auth.js provider list.
 *
 * Written after production spent a day serving a sign-in page whose only
 * working button was GitHub. The deployed build had grown an email
 * "magic link" provider, and the runtime log filled with:
 *
 *   [auth][error] UnknownAction: Cannot handle action: verify-request
 *   [auth][error] Verification: ...#verification
 *
 * That is not a typo in a callback URL. Auth.js stores the emailed token in a
 * `VerificationToken` row and reads it back when the link is clicked, which it
 * can only do through a database adapter. `auth.ts` is `strategy: "jwt"` with
 * no adapter at all, so an email provider cannot work here by construction —
 * every link it sends is dead on arrival, and nothing fails at build time to
 * say so.
 *
 * The `VerificationToken` model exists in schema.prisma, which makes the trap
 * worse: the table is right there, so wiring the provider up looks finished.
 * Adding one requires also passing an adapter (`@auth/prisma-adapter`), and
 * with an adapter the session strategy needs deciding too — the kind of change
 * that should be made deliberately, not discovered from a production log.
 *
 * Checked by reading the source rather than importing it. `auth.ts` pulls in
 * the Auth.js Next.js runtime, which reaches for `next/server` and does not
 * resolve under vitest — the same reason `middleware.test.ts` parses its
 * matcher out of the file text. A static check is weaker than a real import,
 * but it catches the thing that actually went wrong: a provider added to this
 * file without the adapter it needs.
 */

const AUTH_SOURCE = readFileSync(join(__dirname, "auth.ts"), "utf8");

/** Strip comments so prose about email providers can't trip the regexes. */
const CODE = AUTH_SOURCE.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

/** Provider imports that only work with a database adapter behind them. */
const ADAPTER_BOUND_PROVIDERS =
  /from\s+["']next-auth\/providers\/(email|nodemailer|resend|sendgrid|postmark|mailgun|forwardemail)["']/;

describe("auth providers", () => {
  it("registers no magic-link provider without an adapter to store its tokens", () => {
    const usesEmailProvider = ADAPTER_BOUND_PROVIDERS.test(CODE);
    const hasAdapter = /\badapter\s*:/.test(CODE);

    expect(
      usesEmailProvider && !hasAdapter,
      "auth.ts imports an email/magic-link provider but configures no `adapter`. " +
        "Auth.js persists the emailed token through the adapter, so every link it sends " +
        'fails with "Cannot handle action: verify-request" — this is exactly the bug that ' +
        "reached production. Add an adapter (and revisit the session strategy), or drop the provider.",
    ).toBe(false);
  });

  it("uses JWT sessions, which is what the absent adapter implies", () => {
    // Not a style preference: with no adapter there is nowhere to persist a
    // database session, so anything else here is a misconfiguration.
    expect(CODE).toMatch(/strategy:\s*["']jwt["']/);
  });

  it("keeps every provider gated on its own credentials", () => {
    // A provider registered unconditionally shows a button that cannot work
    // when its env vars are absent — production logged
    // `Provider with id "google" not found` for precisely this reason.
    for (const [provider, envVar] of [
      ["GitHub", "AUTH_GITHUB_ID"],
      ["Google", "AUTH_GOOGLE_ID"],
    ] as const) {
      const registration = new RegExp(
        `process\\.env\\.${envVar}[\\s\\S]{0,120}?providers\\.push\\(${provider}\\)`,
      );
      expect(
        registration.test(CODE),
        `${provider} must only be registered when ${envVar} is set.`,
      ).toBe(true);
    }
  });

  it("sends unauthenticated visitors to the custom sign-in page", () => {
    // `/signin` is a real route carrying the provider buttons and the
    // "no sign-in method is configured" notice. Losing this falls back to
    // Auth.js's built-in page, which has neither.
    expect(CODE).toMatch(/pages:\s*\{\s*signIn:\s*["']\/signin["']/);
  });
});
