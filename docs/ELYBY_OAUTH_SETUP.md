# Ely.by sign-in for ComfyCraft

ComfyCraft uses Ely.by OAuth 2.0 rather than collecting an account password in the launcher.

## Deployed Cloudflare Worker

The OAuth broker has been created in the connected Cloudflare account:

- Worker: `comfycraft-ely-auth`
- Base URL: `https://comfycraft-ely-auth.sartywerty15664.workers.dev`
- Authorization start: `POST https://comfycraft-ely-auth.sartywerty15664.workers.dev/v1/auth/start`
- OAuth callback: `https://comfycraft-ely-auth.sartywerty15664.workers.dev/v1/auth/callback`
- Poll result: `GET https://comfycraft-ely-auth.sartywerty15664.workers.dev/v1/auth/poll?session=SESSION_ID`
- Durable Object namespace: `comfycraft-ely-auth_OAuthSession` (SQLite-backed)

The Worker and Durable Object are deployed. The workers.dev endpoint is enabled. Ely.by credentials have **not** been configured yet, so sign-in is not live until the application is registered and the two credentials below are added.

## Ely.by application

Register a new OAuth application in Ely.by:

1. Open the Ely.by developer applications page and create an application.
2. Select application type **Website**.
3. Set the Redirect URI to this exact value (no trailing slash):

   `https://comfycraft-ely-auth.sartywerty15664.workers.dev/v1/auth/callback`

4. Save the application and open its details. Ely.by will show `clientId` and `clientSecret`.

Official OAuth documentation: https://docs.ely.by/ru/oauth.html

## Cloudflare variables and secret

In Cloudflare Dashboard, open **Workers & Pages → comfycraft-ely-auth → Settings → Variables and Secrets**.

Add the following:

- Variable name: `ELY_CLIENT_ID`
- Variable value: the `clientId` shown by Ely.by.
- Secret name: `ELY_CLIENT_SECRET`
- Secret value: the `clientSecret` shown by Ely.by.

The non-secret `ELY_REDIRECT_URI` variable is already configured on the Worker with the exact callback URL above. The `OAUTH_SESSIONS` Durable Object binding and SQLite migration are also deployed.

**Never put `clientSecret` in GitHub, source code, frontend code, Minecraft arguments, or chat.** Enter it directly into Cloudflare as a Secret. If it is ever exposed, rotate it in Ely.by.

## OAuth flow

The Worker requests these Ely.by scopes:

- `account_info`
- `minecraft_server_session`
- `offline_access`

Endpoints:

- `POST /v1/auth/start` returns `sessionId` and `authorizationUrl`.
- Open `authorizationUrl` in the user's system browser.
- Poll `GET /v1/auth/poll?session=SESSION_ID` at a modest interval until it returns `202 pending`, `410 expired`, or `200 complete`.
- The callback exchanges the one-time authorization code server-side, reads the account UUID/username, and stores the result briefly in the session Durable Object.
- The poll result is one-time and the session expires after 10 minutes.

The official Ely.by OAuth documentation confirms the authorization URL, token endpoint, account-info endpoint, and these scopes.

## Desktop integration contract

The Rust desktop client still needs its OS-keyring integration, browser opening, polling, logout/refresh handling, and launch-time token injection before the Play action can safely start Minecraft. Tokens must be stored in the operating system credential manager and never written to logs, instance metadata, crash reports, or the repository.

The launcher must remain launch-disabled until those pieces are implemented and tested. The Worker has been deployed, but the full Ely.by sign-in flow has not yet been end-to-end tested with a real Ely.by application.
