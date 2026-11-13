# ComfyCraft Multicore Runtime

## Scope of the first implementation

This milestone does **not** rewrite Minecraft's game engine. The launcher cannot safely make arbitrary game-state operations concurrent by itself. Minecraft owns mutable client and world state on designated game/server threads; background workers return computed results to those owner threads.

The first milestone adds a runtime planner for the vanilla Java 26.3 background executor. It exposes three profiles and produces the JVM system property `-Dmax.bg.threads=N`. The value is a ceiling for Minecraft's background pool, not a command to create CPU cores or a guarantee of higher FPS.

## What the profiles mean

- **conservative**: smaller worker ceiling, intended to leave more CPU headroom for the game thread and other applications.
- **balanced**: vanilla-style worker ceiling based on the logical processors visible to the JVM.
- **maximum**: the highest ceiling vanilla's own CPU-based sizing can use. This cannot exceed the CPU budget visible to Java.

On a four-logical-processor CPU, balanced and maximum may resolve to the same value. That is expected: a launcher setting cannot exceed the engine's own processor clamp.

## Next engine research targets

1. Measure worker-pool saturation and time spent waiting on chunk futures.
2. Profile the chunk generation stages that already fork work to the background pool.
3. Investigate serial stages only after proving they are a bottleneck and documenting their read/write radius and thread ownership.
4. Keep world-state commits on the owning server/client thread. Never mutate live chunks, entities, or GPU state from arbitrary workers.
5. Compare identical seeds, view distances, JVM versions, and hardware with repeatable benchmarks.

## Acceptance criteria

- Planner output is deterministic for a given CPU visibility and profile.
- Unknown profiles are rejected.
- Generated JVM arguments are surfaced to the launch-argument inspector.
- No profile claims to guarantee FPS gains.
- Actual launch integration remains disabled until authentication and process supervision are implemented.
