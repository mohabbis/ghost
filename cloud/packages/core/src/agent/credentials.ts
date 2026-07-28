import { createHash, randomBytes } from "node:crypto";

const TOKEN_PREFIX = "ghost_agent_";

export function hashAgentToken(token: string): string {
  return createHash("sha256").update(token, "utf8").digest("hex");
}

export function createAgentToken(): {
  token: string;
  tokenHash: string;
  tokenHint: string;
} {
  const token = `${TOKEN_PREFIX}${randomBytes(32).toString("base64url")}`;
  return {
    token,
    tokenHash: hashAgentToken(token),
    tokenHint: `${TOKEN_PREFIX}…${token.slice(-6)}`,
  };
}
