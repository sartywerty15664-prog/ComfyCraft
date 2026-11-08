export interface Env {
  ELY_CLIENT_ID: string;
  ELY_CLIENT_SECRET: string;
  ELY_REDIRECT_URI: string;
  OAUTH_SESSIONS: DurableObjectNamespace;
}

type TokenResponse = {
  access_token: string;
  refresh_token?: string;
  token_type: string;
  expires_in: number;
  scope?: string;
};

type AccountInfo = {
  uuid: string;
  username: string;
};

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  });
}

function randomHex(bytes = 32): string {
  const data = new Uint8Array(bytes);
  crypto.getRandomValues(data);
  return [...data].map((value) => value.toString(16).padStart(2, "0")).join("");
}

function html(title: string, message: string): Response {
  const safeTitle = title.replace(/[&<>"]/g, "");
  const safeMessage = message.replace(/[&<>"]/g, "");
  return new Response(`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>${safeTitle}</title><body style="font:16px system-ui;max-width:36rem;margin:12vh auto;padding:1.5rem;background:#10131b;color:#f4f6fb"><h1>${safeTitle}</h1><p>${safeMessage}</p><p>You can close this tab and return to ComfyCraft.</p></body></html>`, {
    headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" },
  });
}

export class OAuthSession {
  private state?: DurableObjectState;
  constructor(state: DurableObjectState) { this.state = state; }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    if (request.method === "POST" && url.pathname === "/initialize") {
      const body = await request.json<{ state: string }>();
      if (typeof body.state !== "string" || body.state.length < 32 || body.state.length > 256) return json({ error: "invalid_state" }, 400);
      await this.state!.storage.put("expected_state", body.state);
      await this.state!.storage.put("created_at", Date.now());
      return json({ ok: true });
    }
    if (request.method === "POST" && url.pathname === "/complete") {
      const body = await request.json<{ state: string; result: TokenResponse & { account: AccountInfo } }>();
      const expected = await this.state!.storage.get<string>("expected_state");
      const created = await this.state!.storage.get<number>("created_at");
      if (!expected || expected !== body.state || !created || Date.now() - created > 10 * 60_000) return json({ error: "invalid_or_expired_state" }, 400);
      if (!body.result?.access_token || !body.result?.account?.uuid || !body.result?.account?.username) return json({ error: "invalid_token_result" }, 400);
      await this.state!.storage.put("result", body.result);
      await this.state!.storage.put("completed_at", Date.now());
      await this.state!.storage.delete("expected_state");
      return json({ ok: true });
    }
    if (request.method === "GET" && url.pathname === "/poll") {
      const created = await this.state!.storage.get<number>("created_at");
      if (!created || Date.now() - created > 10 * 60_000) return json({ status: "expired" }, 410);
      const result = await this.state!.storage.get<TokenResponse & { account: AccountInfo }>("result");
      if (!result) return json({ status: "pending" }, 202);
      await this.state!.storage.deleteAll();
      return json({ status: "complete", ...result });
    }
    return json({ error: "not_found" }, 404);
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/v1/auth/start") {
      if (!env.ELY_CLIENT_ID || !env.ELY_CLIENT_SECRET || !env.ELY_REDIRECT_URI) return json({ error: "oauth_not_configured" }, 503);
      const sessionId = randomHex(24);
      const state = randomHex(32);
      const id = env.OAUTH_SESSIONS.idFromName(sessionId);
      const stub = env.OAUTH_SESSIONS.get(id);
      const initialized = await stub.fetch("https://session/initialize", {
        method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ state }),
      });
      if (!initialized.ok) return json({ error: "session_initialization_failed" }, 502);
      const authorize = new URL("https://account.ely.by/oauth2/v1");
      authorize.searchParams.set("client_id", env.ELY_CLIENT_ID);
      authorize.searchParams.set("redirect_uri", env.ELY_REDIRECT_URI);
      authorize.searchParams.set("response_type", "code");
      authorize.searchParams.set("scope", "account_info minecraft_server_session offline_access");
      authorize.searchParams.set("state", `${sessionId}.${state}`);
      return json({ sessionId, authorizationUrl: authorize.toString(), expiresIn: 600 });
    }

    if (request.method === "GET" && url.pathname === "/v1/auth/callback") {
      const code = url.searchParams.get("code");
      const packedState = url.searchParams.get("state");
      if (!code || !packedState || !/^[a-f0-9]{48}\.[a-f0-9]{64}$/.test(packedState)) return html("Sign-in failed", "The authorization response was incomplete.");
      const [sessionId, state] = packedState.split(".");
      const stub = env.OAUTH_SESSIONS.get(env.OAUTH_SESSIONS.idFromName(sessionId));
      const tokenResponse = await fetch("https://account.ely.by/api/oauth2/v1/token", {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          client_id: env.ELY_CLIENT_ID,
          client_secret: env.ELY_CLIENT_SECRET,
          redirect_uri: env.ELY_REDIRECT_URI,
          grant_type: "authorization_code",
          code,
        }),
      });
      if (!tokenResponse.ok) return html("Sign-in failed", "Ely.by did not approve the token exchange. You can close this tab and try again.");
      const token = await tokenResponse.json<TokenResponse>();
      if (!token.access_token) return html("Sign-in failed", "Ely.by returned an incomplete token response.");
      const accountResponse = await fetch("https://account.ely.by/api/account/v1/info", {
        headers: { authorization: `Bearer ${token.access_token}` },
      });
      if (!accountResponse.ok) return html("Sign-in failed", "The account profile could not be verified.");
      const account = await accountResponse.json<AccountInfo>();
      const completed = await stub.fetch("https://session/complete", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ state, result: { ...token, account: { uuid: account.uuid, username: account.username } } }),
      });
      if (!completed.ok) return html("Sign-in expired", "The ComfyCraft sign-in session expired. Start sign-in again from the launcher.");
      return html("Ely.by connected", `Signed in as ${account.username}. Return to ComfyCraft to finish.`);
    }

    if (request.method === "GET" && url.pathname === "/v1/auth/poll") {
      const sessionId = url.searchParams.get("session");
      if (!sessionId || !/^[a-f0-9]{48}$/.test(sessionId)) return json({ error: "invalid_session" }, 400);
      const stub = env.OAUTH_SESSIONS.get(env.OAUTH_SESSIONS.idFromName(sessionId));
      return stub.fetch("https://session/poll");
    }

    return json({ error: "not_found" }, 404);
  },
};
