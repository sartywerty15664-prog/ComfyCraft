# Rust Core milestone: parallel asset verification

## Delivered in this milestone

- Added a native Rust asset-cache verifier for an installed Minecraft instance.
- Reads the exact asset-index ID from the installed Minecraft version metadata.
- Validates each object path, declared size, and SHA-1 digest.
- Verifies independent object files concurrently using a bounded pool of up to 16 native worker threads.
- Returns totals, missing/corrupt counts, verified bytes, worker count, elapsed time, and a small diagnostic sample.
- Exposes the verifier to the Play screen through the `VERIFY ASSET CACHE` action.

## Thread-safety boundary

The worker pool only reads immutable files and computes hashes. It does not access Minecraft world state, renderer state, Java objects, or GPU resources. This is a real parallel Rust subsystem, but it is not a claim that the proprietary Minecraft Java engine has been rewritten.

## Next implementation stages

1. Add repair/download for missing or corrupt asset objects using the index's official object URL and SHA-1.
2. Build a metadata-driven launch plan that merges vanilla 26.3 and Fabric 0.19.5 libraries without bundling unrelated versions.
3. Add a process supervisor that starts Java with an argument vector (never a shell command string), streams logs, tracks exit status, and supports graceful shutdown.
4. Implement Ely.by authentication only through a supported public-client/native flow or a trusted server-side exchange. Never embed a client secret in the desktop binary or store passwords in plaintext.
5. Profile the actual 26.3 runtime before making engine-level concurrency patches. Preserve render-thread and world-thread ownership, and benchmark p50/p95/p99 frame times and chunk throughput.

## Validation still required

The new Rust module and frontend integration need a successful Linux CI run and a CachyOS/Arch smoke test. The first verification run should be performed on a fully installed instance; it will report missing files rather than silently downloading replacements.
