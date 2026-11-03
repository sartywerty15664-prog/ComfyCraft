# Minecraft launch pipeline

Installation and launching are separate operations. A successful download is not proof that the game can launch.

## Required stages

1. Resolve the selected instance and read its official version metadata.
2. Evaluate library OS, architecture and feature rules; include only applicable artifacts.
3. Verify the client JAR, required library artifacts, asset index, asset objects and extracted Linux natives.
4. Resolve Java and enforce the version required by the selected Minecraft metadata.
5. Parse the structured JVM and game argument arrays, including conditional entries and placeholders.
6. Supply game directory, assets directory/index, natives directory, launcher identity and authenticated profile values.
7. Spawn Java using an argument vector, not a shell; stream output to the instance log and report its exit code.
8. Do not launch until Ely.by authentication and profile/session data are available.

## Ely.by authentication requirements

The official Ely.by OAuth 2.0 flow uses the authorization endpoint `https://account.ely.by/oauth2/v1`, the callback parameters `code` and `state`, and the token endpoint `https://account.ely.by/api/oauth2/v1/token`. Request the scopes `account_info minecraft_server_session offline_access`. Validate `state`, exchange the code using the exact registered redirect URI, and obtain the account profile from `https://account.ely.by/api/account/v1/info`.

Important security constraint: Ely.by's documented authorization-code exchange requires a registered `client_secret`. A desktop binary is not a safe place to embed a client secret. Before implementing the exchange, the project needs an approved architecture for handling that secret (for example, a trusted server-side component) or confirmation from Ely.by of a supported public/native-client flow. Do not ship a guessed or exposed secret. The OAuth app and redirect URI must be registered by the project owner.

Store refresh credentials in the desktop OS credential store. Never write tokens to instance metadata, logs, source control or crash reports. Password collection is out of scope.

## Current implementation boundary

The installer downloads official Mojang metadata, client, libraries and assets. Runtime inspection and preflight checks exist. The metadata argument inspector now resolves known placeholders, exposes the JVM and game argument vectors for validation, reports unresolved placeholders, and includes the required Java major version plus detected runtime version. It still does not launch Java. Ely.by callback/token handling, secure credential storage, final authentication argument injection and Java process supervision remain pending. The UI must not claim Play works until those stages are implemented and tested on Linux.


## Metadata rule evaluation

Linux library and argument rules are evaluated in their declared order, including OS name, architecture, and feature predicates. Launcher-only feature flags are explicitly false until their corresponding settings and values are implemented; unknown feature keys do not silently pass. Unit tests cover rule ordering, default behavior, and feature matching.


## Java runtime discovery

The Linux runtime manager checks `JAVA_HOME/bin/java`, Java executables available through `PATH`, and common JVM locations under `/usr/lib/jvm`, `/usr/java` and `/opt/java`. Candidate binaries are canonicalized and deduplicated, then queried with `java -version`. Both modern version strings (for example, 25.0.1) and legacy 1.x strings are parsed. Settings lists detected runtimes and marks Java 25+ as compatible with the current 26.3 target. ComfyCraft does not silently download a runtime or execute an installer.


## Java runtime selection

Settings allows choosing one of the detected Java executables. The selected canonical executable path is saved in the application data directory, and runtime diagnostics plus preflight checks use that selection rather than an unrelated `java` found first on PATH. A saved selection is revalidated against the current detected runtimes; if it is no longer available, the manager falls back to the first detected runtime.
