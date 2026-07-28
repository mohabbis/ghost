/**
 * Thin HTTP client for Ghost Cloud agent API.
 * Used by the MCP stdio bridge — never calls approve endpoints.
 */

export type GhostAgentClientOptions = {
  baseUrl: string;
  apiKey: string;
};

export class GhostAgentClient {
  private readonly baseUrl: string;
  private readonly apiKey: string;

  constructor(opts: GhostAgentClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/$/, "");
    this.apiKey = opts.apiKey;
  }

  async catalog(): Promise<unknown> {
    return this.request("GET", "/api/agent");
  }

  async invoke(name: string, args: Record<string, unknown> = {}): Promise<unknown> {
    return this.request("POST", "/api/agent/invoke", { name, arguments: args });
  }

  private async request(method: string, path: string, body?: unknown): Promise<unknown> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        authorization: `Bearer ${this.apiKey}`,
        ...(body !== undefined ? { "content-type": "application/json" } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    const text = await res.text();
    let data: unknown = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch {
      data = { raw: text };
    }
    if (!res.ok) {
      const err =
        data && typeof data === "object" && "error" in data
          ? String((data as { error: unknown }).error)
          : `HTTP ${res.status}`;
      throw new Error(err);
    }
    return data;
  }
}
