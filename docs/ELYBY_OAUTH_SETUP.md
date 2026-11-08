# Ely.by sign-in for ComfyCraft

ComfyCraft uses Ely.by OAuth 2.0 rather than collecting an account password in the launcher.

## Why the broker exists

Ely.by's documented authorization-code exchange requires `client_secret`. A desktop binary is public and can be inspected, so the secret must not be embedded in Rust, Java arguments, frontend bundles, or GitHub files. The Cloudflare Worker in `auth-worker/` keeps the exchange server-side.

The worker requests only these scopes:

- `account_info`
- `minecraft_server_session`
- `offline_access`

It uses a random state value, a short-lived Durable Object session, and a one-time poll response. The access and refresh tokens are returned to the local launcher only after the callback is verified. The launcher must store tokens in the operating system credential store and must never write them to logs, instance metadata, crash reports, or the repository.

## Ely.by application

1. Register an Ely.by OAuth application as a website.
2. Set its redirect URI to the exact HTTPS callback URL:
   `https://YOUR-WORKER-DOMAIN/v1/auth/callback`
3. Copy the client ID. Keep the client secret private.

## Cloudflare Worker configuration

From `auth-worker/`, install Wrangler and deploy the worker. Configure:

- `ELY_CLIENT_ID` as a Worker variable.
- `ELY_REDIRECT_URI` as the exact callback URL registered at Ely.by.
- `ELY_CLIENT_SECRET` as a Wrangler secret (never as a committed variable).
- The `OAUTH_SESSIONS` Durable Object binding is declared in `wrangler.toml`.

Example commands (replace the values locally; do not paste the secret into chat or GitHub):

```sh
npx wrangler secret put ELY_CLIENT_SECRET
npx wrangler deploy
```

Set the non-secret values in the Cloudflare Worker settings or with Wrangler before deployment.

## Desktop integration contract

- `POST /v1/auth/start` returns `sessionId` and `authorizationUrl`.
- Open `authorizationUrl` in the user's system browser.
- Poll `GET /v1/auth/poll?session=SESSION_ID` at a modest interval until it returns `202 pending`, `410 expired`, or `200 complete`.
- On completion, validate the account UUID and store tokens in the OS credential manager. Do not store tokens in localStorage or ordinary JSON files.
- Add the authlib-injector javaagent to the Java argument vector and use Ely.by's authlib-injector endpoint configuration for the game session.

The worker is an OAuth broker, not a token vault: the one-time poll clears its stored result. Production deployment should additionally apply Cloudflare rate limiting and monitor only metadata, never token payloads.

## Current implementation boundary

This worker is the server-side OAuth component. The Rust desktop client still needs its OS-keyring integration, browser opening, polling, logout/refresh handling, and launch-time token injection before the Play action can safely start Minecraft. The launcher must remain launch-disabled until those pieces are implemented and tested.
