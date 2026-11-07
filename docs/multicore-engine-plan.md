# ComfyCraft Multicore Engine — engineering plan

## Current verified baseline

Minecraft Java 26.3 already has a shared background `ForkJoinPool` used by chunk generation, lighting, section meshing, resource-reload preparation, and chunk serialization. Its worker count is based on the processor count visible to the JVM, with a lower reserve and the `max.bg.threads` system-property ceiling.

ComfyCraft's Rust runtime planner now:
- detects the logical processor count visible to the launcher;
- exposes conservative, balanced, and maximum worker ceilings;
- adds one `-Dmax.bg.threads=N` argument to the launch-argument inspection output;
- reports the selected plan for diagnostics.

This is a real launcher-side runtime change, **not yet a rewrite of Minecraft's Java engine**. The existing launch pipeline still marks launching disabled until authentication and process supervision are implemented.

## Engine architecture target

The goal is to improve throughput without breaking Minecraft's thread-ownership model:

- **Render thread** owns mutable client/render state.
- **Server thread** owns mutable world state.
- **Background workers** may calculate from safe inputs and return immutable/results via futures.
- **IO workers** handle storage operations and preserve ordering where required.

Workers must not directly mutate live world state, entities, render state, or GPU resources owned by another thread. Work should return to the owning executor for commit.

## Candidate optimization areas

1. **Chunk generation** — profile stage durations and queue wait time first. Split only independent, deterministic work with explicit input snapshots and output buffers.
2. **Lighting** — identify independent section/region calculations; preserve boundary propagation and ordering requirements.
3. **Section meshing** — parallelize CPU-side mesh preparation only; keep GPU upload and render-resource mutation on the render thread.
4. **Serialization** — parallelize independent compression/encoding work while preserving per-region write ordering.
5. **Resource reload** — parallelize prepare phases that do not mutate live registries, then commit on the owning thread.

## Required implementation method

- Use the exact official 26.3 client/runtime metadata and matching symbols for analysis.
- Keep any proprietary client binaries and decompiled source local to the developer's machine; do not commit or redistribute Mojang-owned code.
- Implement changes as a separately versioned patch/agent or a legally distributable clean-room component. Do not silently patch arbitrary classes based on guessed names.
- Add concurrency tests for determinism, cancellation, shutdown, exception propagation, and race-free handoff.
- Benchmark identical worlds, seeds, view distances, Java versions, and hardware against an unmodified client.
- Keep a safe fallback to vanilla behavior for any optimization that fails validation.

## Performance acceptance criteria

Report more than average FPS:
- chunk generation throughput (chunks/second);
- time-to-visible terrain after teleport/travel;
- worker-pool utilization and queue wait;
- main/server tick duration and p95/p99 stalls;
- frame-time p50/p95/p99;
- memory use and CPU utilization.

A higher thread count alone is not a success criterion. A change is accepted only when repeatable measurements show an improvement without world corruption, crashes, deadlocks, or worse tail latency.

## Current blockers

- Minecraft's Java engine is not part of this repository and is not open-source.
- The launcher does not yet implement authenticated process launch/supervision.
- No baseline benchmark or target-device profile has been captured yet.
- A true engine patch requires inspection of the exact 26.3 runtime and focused experiments; it cannot be honestly claimed complete from Rust launcher code alone.
