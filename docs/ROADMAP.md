# Roadmap

## Milestone 0 — desktop foundation
- [x] Linux-only Tauri 2 + React shell
- [x] Play / Instances / Mods / Settings navigation
- [ ] Build and smoke-test on CachyOS/Arch, KDE Plasma and Hyprland

## Milestone 1 — instance foundation
- [x] Managed instance directory layout
- [x] Create, rename and remove instances
- [x] Persistent JSON metadata and validated instance identifiers
- [x] Isolated mods, resourcepacks, shaderpacks, saves and logs directories

## Milestone 2 — game installation
- [x] Resolve official Mojang version manifest and list stable releases in the UI
- [x] Resolve and SHA-1 verify per-version metadata
- [x] Download the official client JAR, Linux libraries and native archives
- [x] Download asset index and referenced asset objects
- [x] Verify declared SHA-1 hashes and file sizes; reuse valid cached files
- [x] Persist an installation marker per instance
- [x] Extract Linux native archives safely and build the runtime classpath
- [ ] Download/validate logging configuration and complete installer smoke tests
- [x] Detect Java runtimes from JAVA_HOME, PATH and common Linux JVM directories
- [x] Parse modern and legacy Java version strings with unit tests
- [x] Show detected runtimes and Java 25 compatibility in Settings
- [x] Select and persist the Java executable used by runtime diagnostics

## Milestone 3 — modded gameplay
- [x] Install/update stable Fabric Loader compatible with the instance Minecraft version
- [x] Modrinth mod search filtered by Minecraft version and Fabric
- [x] Install compatible mod versions and recursively resolve required project dependencies
- [x] Verify downloaded mod artifacts against Modrinth SHA-1 hashes
- [x] Track managed mod files and safely uninstall only files owned by ComfyCraft
- [x] Check for newer compatible mod versions and install updates
- [ ] Pin exact dependency version IDs and handle incompatible/optional dependency choices
- [ ] One-click performance profile for Sodium, Lithium, FerriteCore and compatible alternatives

## Milestone 4 — graphics and accounts
- [ ] Ely.by supported sign-in flow and Linux secret-service token storage
- [ ] Validate Iris compatibility with the selected Fabric and Minecraft versions
- [x] Search, install and safely uninstall Modrinth resource packs
- [x] Search, install and safely uninstall Modrinth shader packs
- [ ] Enable/disable resource packs and shader packs from the launcher

## Milestone 5 — launch and distribution
- [x] Preflight checks for Java, client, natives, assets and libraries
- [x] Structured JVM/game argument expansion and placeholder inspection
- [ ] Java process supervision after authenticated profile/session data is available
- [ ] Live logs and useful crash diagnostics
- [x] Linux package bundle targets configured (AppImage, deb and rpm)
- [ ] Build and smoke-test Linux packages on target distributions
- [ ] Wayland/X11 and NVIDIA testing

## External prerequisites
- Ely.by OAuth authorization-code exchange requires a registered application and a confidential client secret. A desktop binary must not contain that secret. The project needs a trusted server-side exchange component or written confirmation of a supported public/native-client flow before account sign-in can be completed.
- The owner must register the OAuth application and callback URI. No password collection or authentication bypass is planned.

No milestone should claim completion until it has been built and tested on Linux. GitHub Actions checks validate compilation; they do not replace testing on a real CachyOS/Arch desktop.
