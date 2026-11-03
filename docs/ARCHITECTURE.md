# Architecture

## High-level design

ComfyCraft is a Linux desktop application. The React frontend is a presentation layer; Rust owns filesystem access, process management, downloads, integrity checks and credential integration.

```
React UI
  | Tauri IPC (validated commands)
Rust application services
  |-- Instance registry and filesystem
  |-- Minecraft metadata / artifact resolver
  |-- Java runtime detection
  |-- Modrinth API client and dependency graph
  |-- Loader adapters (Fabric first)
  |-- Account provider adapter (Ely.by)
  |-- Process launcher and log streaming
Linux desktop / filesystem / secret service
```

## Security boundaries

- Never expose arbitrary filesystem or shell commands to the frontend.
- Restrict file operations to managed instance directories unless the user explicitly selects a location.
- Validate URLs and artifact checksums before installation.
- Do not persist access tokens in plain text or logs; integrate with a Linux secret service.
- Avoid shell string concatenation when launching Java; pass structured process arguments.
- Keep Minecraft assets and proprietary libraries out of the repository.

## Planned data model

An instance has a stable ID, display name, Minecraft version, loader profile, Java selection, game directory, enabled mods and resource-pack state. Instance changes should be staged and validated before being committed so a failed dependency install cannot leave a half-updated profile.

## Compatibility

Linux is the only supported platform. First-class testing targets are CachyOS/Arch, KDE Plasma and Hyprland, Wayland and X11, with NVIDIA graphics as an initial performance target.
