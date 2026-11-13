use serde::Serialize;

/// Vanilla Minecraft 26.3 sizes its shared background pool from the JVM's
/// available processor count and applies max.bg.threads as an upper bound.
/// This planner never pretends that the property can create CPU cores.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPlan {
    pub profile: String,
    pub logical_processors: usize,
    pub minecraft_worker_limit: usize,
    pub jvm_arguments: Vec<String>,
    pub explanation: String,
}

pub fn build_plan(profile: &str) -> Result<ThreadPlan, String> {
    let logical = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .max(1);

    // Vanilla's worker pool normally reserves at least one logical processor
    // for the game/client and clamps its background executor accordingly.
    let vanilla_ceiling = logical.saturating_sub(1).max(1);
    let (normalized, requested) = match profile {
        "conservative" => ("conservative", (logical + 1) / 2),
        "balanced" => ("balanced", vanilla_ceiling),
        "maximum" => ("maximum", vanilla_ceiling),
        other => return Err(format!(
            "Unknown multicore profile '{other}'. Use conservative, balanced, or maximum."
        )),
    };
    let workers = requested.max(1).min(vanilla_ceiling);

    let explanation = match normalized {
        "conservative" => "Uses a smaller background-worker ceiling to leave more CPU time for the game thread and other apps.",
        "maximum" if logical <= 2 => "This CPU exposes at most two logical processors; Minecraft's vanilla worker-pool ceiling cannot be raised beyond the available CPU budget.",
        "maximum" => "Uses the highest background-worker ceiling vanilla Minecraft can use for the processors visible to Java. Raising this property further cannot create more parallelism.",
        _ => "Uses the vanilla background-worker ceiling for the processors visible to Java, while leaving one logical processor outside the worker budget.",
    };

    Ok(ThreadPlan {
        profile: normalized.to_string(),
        logical_processors: logical,
        minecraft_worker_limit: workers,
        jvm_arguments: vec![format!("-Dmax.bg.threads={workers}")],
        explanation: explanation.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_the_vanilla_background_pool_property() {
        let plan = build_plan("balanced").unwrap();
        assert!(plan.jvm_arguments[0].starts_with("-Dmax.bg.threads="));
        assert!(plan.minecraft_worker_limit >= 1);
        assert!(plan.minecraft_worker_limit <= plan.logical_processors.max(1));
    }

    #[test]
    fn rejects_unknown_profiles() {
        assert!(build_plan("turbo-9000").is_err());
    }

    #[test]
    fn conservative_profile_never_exceeds_balanced() {
        let conservative = build_plan("conservative").unwrap();
        let balanced = build_plan("balanced").unwrap();
        assert!(conservative.minecraft_worker_limit <= balanced.minecraft_worker_limit);
    }

    #[test]
    fn maximum_profile_respects_vanilla_cpu_ceiling() {
        let maximum = build_plan("maximum").unwrap();
        assert!(maximum.minecraft_worker_limit <= maximum.logical_processors.saturating_sub(1).max(1));
    }
}
