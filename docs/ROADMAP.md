# Roadmap

## Milestone 0 — desktop foundation
- [x] Linux-only Tauri 2 + React shell
- [x] Play / Instances / Mods / Settings navigation
- [ ] Build and smoke-test on CachyOS/Arch

## Milestone 1 — instance foundation
- [ ] Managed instance directory layout
- [ ] Create, rename, duplicate and remove instances
- [ ] Persistent metadata and safe filesystem boundaries

## Milestone 2 — game installation
- [ ] Resolve official version manifest and per-version metadata
- [ ] Download client, libraries, assets and logging configuration
- [ ] Verify SHA-1 / available checksums and file sizes
- [ ] Detect Java and validate the required runtime for each version

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
- [ ] Preflight checks and structured Java process arguments
- [ ] Live logs and useful crash diagnostics
- [ ] AppImage and Arch package
- [ ] Wayland/X11 and NVIDIA testing

No milestone should claim completion until it has been built and tested on Linux.
