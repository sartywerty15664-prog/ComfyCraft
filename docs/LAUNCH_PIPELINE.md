# Minecraft launch pipeline

Installation and launching are separate operations. A successful download is not proof that the game can launch.

## Required stages

1. Resolve the selected instance and read its official version metadata.
2. Evaluate library OS, architecture and feature rules; include only applicable artifacts.
3. Verify the client JAR, required library artifacts, asset index, asset objects and extracted Linux natives.
4. Resolve Java and enforce the exact `javaVersion.majorVersion` from metadata.
5. Expand the metadata's structured JVM and game argument arrays, including conditional entries and placeholders.
6. Supply game directory, assets directory/index, natives directory, launcher identity and authenticated profile values.
7. Spawn Java using an argument vector, not a shell; stream output to the instance log and report its exit code.
8. Do not launch until Ely.by authentication and profile/session data are available.

## Ely.by authentication requirements

Use Ely.by OAuth 2.0 with `account_info minecraft_server_session offline_access`. Open the authorization page in the system browser, validate the returned `state`, exchange the authorization code, fetch the account profile and refresh credentials when needed. Store refresh credentials in the desktop OS credential store. Never write tokens to instance metadata, logs, source control or crash reports.

The OAuth application must be registered by the project owner. Client credentials and the exact redirect URI are deployment configuration and must not be guessed or hard-coded. Password collection is out of scope.

## Current boundary

The installer downloads official Mojang metadata, client, libraries and assets. Runtime inspection and preflight checks exist. Full argument expansion, OAuth callback handling, secure credential storage and process supervision remain pending. The UI must not claim Play works until those stages are implemented and tested on Linux.
