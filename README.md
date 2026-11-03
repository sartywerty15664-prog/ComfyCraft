# ComfyCraft

A Linux-only Minecraft Java launcher focused on comfortable instance management, mods, shaders and performance.

> Early development. This repository currently contains the desktop shell and project foundation. Game downloading, authentication and launching are not implemented yet.

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

- [x] Linux-only repository and application shell
- [x] Initial dark UI: Play, Instances, Mods, Settings
- [ ] Persistent instance management
- [ ] Official Minecraft metadata and asset/library downloader
- [ ] Java runtime detection (Minecraft 26.3 target: Java 25; verify against official release metadata)
- [ ] Fabric/other loader installation
- [ ] Modrinth project search and dependency resolution
- [ ] Ely.by authentication using its supported OAuth flow
- [ ] Sodium/Lithium/FerriteCore and compatible performance profiles
- [ ] Iris shaders, resource packs and shader packs
- [ ] Integrity checks and safe launch process
- [ ] Linux packaging (AppImage and Arch package)

## Project principles

- Never bypass Minecraft ownership or authentication checks.
- Download game files from official metadata and verify hashes where available.
- Keep account tokens in the OS credential store; never log or commit secrets.
- Resolve mod dependencies and loader compatibility before changing an instance.
- Do not bundle proprietary Minecraft assets.

See [ROADMAP.md](docs/ROADMAP.md) and [ARCHITECTURE.md](docs/ARCHITECTURE.md).
