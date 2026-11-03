# ComfyCraft

A Linux-only Minecraft Java launcher focused on comfortable instance management, verified game downloads, Fabric mods, resource packs and shader packs.

> Early development. The launcher can prepare isolated instances and install game files, Fabric and compatible Modrinth content. Authenticated game launching is not yet enabled.

## Platform policy

- Linux only: CachyOS / Arch Linux are the first targets.
- KDE Plasma and Hyprland; Wayland and X11.
- NVIDIA is an initial performance-testing target.
- No Windows builds, compatibility layer, CI or support are planned.

## Stack

- Tauri 2 + Rust backend
- React + TypeScript + Vite frontend
- Linux WebKitGTK runtime

## Development prerequisites

Install Node.js 22+, pnpm, Rust stable, and the Linux Tauri development dependencies for your distribution (GTK/WebKitGTK, AppIndicator and build tools).

```bash
pnpm install
pnpm tauri dev
```

## Current status

- [x] Linux-only application shell and dark UI
- [x] Isolated instance management
- [x] Official Minecraft metadata, client, library, native and asset downloads with integrity checks
- [x] Java runtime discovery and persisted runtime selection
- [x] Fabric loader installation
- [x] Modrinth mod search, required dependency installation, managed uninstall and update checks
- [x] Modrinth resource-pack and shader-pack search/install/uninstall
- [x] Launch preflight and structured argument inspection
- [ ] Ely.by sign-in and secure credential storage
- [ ] Actual Java process launch, supervision, live logs and crash diagnostics
- [x] Resource/shader activation controls
- [ ] One-click performance profiles
- [ ] Real CachyOS/Arch smoke tests and package verification

## Project principles

- Never bypass Minecraft ownership or authentication checks.
- Download game files from official metadata and verify hashes where available.
- Keep account tokens in the Linux OS credential store; never log or commit secrets.
- Resolve mod dependencies and loader compatibility before changing an instance.
- Do not bundle proprietary Minecraft assets.
- Never embed an OAuth client secret in the desktop application.

See [ROADMAP.md](docs/ROADMAP.md), [ARCHITECTURE.md](docs/ARCHITECTURE.md) and [LAUNCH_PIPELINE.md](docs/LAUNCH_PIPELINE.md).
