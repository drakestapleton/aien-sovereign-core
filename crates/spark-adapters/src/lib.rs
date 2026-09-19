pub mod distill;
pub mod models;
pub mod providers;
pub mod router;
pub mod vault;
pub mod verifier;

pub use distill::DistillationEngine;
pub use models::*;
pub use router::AdapterRouter;
pub use vault::*;
pub use verifier::Verifier;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_discovery() {
        let router = AdapterRouter::new();
        let adapters = router.discover_adapters();
        assert!(!adapters.is_empty());
        assert!(adapters.iter().any(|a| a.id == "atlas-lightning-omni"));
        assert!(adapters.iter().any(|a| a.id == "openai/gpt-4o"));
        assert!(adapters.iter().any(|a| a.id == "anthropic/claude-3-7-sonnet"));
    }

    #[test]
    fn test_router_resolution() {
        let router = AdapterRouter::new();
        let (prov, endpoint, model, _) = router.resolve_route("anthropic/claude-3-7-sonnet");
        assert_eq!(prov, ProviderType::Anthropic);
        assert!(endpoint.contains("anthropic"));
        assert_eq!(model, "claude-3-7-sonnet-20250219");

        let (prov, _endpoint, model, _) = router.resolve_route("openai/o3-mini");
        assert_eq!(prov, ProviderType::OpenAI);
        assert_eq!(model, "o3-mini");
    }

    #[test]
    fn test_unslop_verification() {
        let clean = "Implementation verified with zero allocations.";
        let out = Verifier::verify(clean, VerificationStrategy::UnslopStrict);
        assert!(out.passed);
        assert_eq!(out.score, 1.0);

        let dirty = "We will delve into this crucial topic.";
        let out_dirty = Verifier::verify(dirty, VerificationStrategy::UnslopStrict);
        assert!(!out_dirty.rule_violations.is_empty());
        assert!(out_dirty.score < 1.0);
    }

    #[test]
    fn test_distill_task_serialization() {
        let task = DistillTask {
            id: "task-001".to_string(),
            task_type: TaskType::CodeSynthesis,
            prompt: "Write an Axum handler for health checks".to_string(),
            system_prompt: Some("You are AIEN Sovereign Architect.".to_string()),
            teacher_model: "anthropic/claude-3-7-sonnet".to_string(),
            student_model: Some("atlas-lightning-omni".to_string()),
            verification_strategy: VerificationStrategy::CompilerCheck,
            commit_to_cortex: true,
        };

        let json_str = serde_json::to_string(&task).expect("serialize");
        let parsed: DistillTask = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(parsed.id, "task-001");
        assert_eq!(parsed.teacher_model, "anthropic/claude-3-7-sonnet");
    }
}
