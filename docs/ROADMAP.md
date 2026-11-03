# Roadmap

## Milestone 0 — desktop foundation
- [x] Linux-only Tauri 2 + React shell
- [x] Play / Instances / Mods / Settings navigation
- [ ] Build and smoke-test on CachyOS/Arch

## Milestone 1 — instance foundation
- [x] Managed instance directory layout
- [x] Create, rename and remove instances
- [x] Persistent JSON metadata and validated instance identifiers

## Milestone 2 — game installation
- [x] Resolve official Mojang version manifest and list stable releases in the UI
- [x] Resolve and SHA-1 verify per-version metadata
- [x] Download the official client JAR
- [x] Download Linux library artifacts and native archives
- [x] Download asset index and all referenced asset objects
- [x] Verify declared SHA-1 hashes and file sizes; reuse valid cached files
- [x] Persist an installation marker per instance
- [x] Extract Linux native archives safely and build the runtime classpath
- [ ] Download/validate logging configuration and complete installer smoke tests
- [x] Detect the installed Java runtime and report its major version

## Milestone 3 — modded gameplay
- [ ] Fabric loader installation
- [ ] Modrinth search, details and version compatibility
- [ ] Dependency graph, required libraries, conflicts and updates
- [ ] Sodium, Lithium, FerriteCore and other compatible optimizations

## Milestone 4 — graphics and accounts
- [ ] Ely.by supported sign-in flow and secure token storage
- [ ] Iris compatibility and shader-pack manager
- [ ] Resource-pack manager with enable/disable controls

## Milestone 5 — launch and distribution
- [x] Preflight checks for Java, client, natives, assets and libraries
- [ ] Structured JVM/game argument expansion and Java process supervision
- [ ] Live logs and useful crash diagnostics
- [ ] AppImage and Arch package
- [ ] Wayland/X11 and NVIDIA testing

No milestone should claim completion until it has been built and tested on Linux.
