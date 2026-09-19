use crate::store::{CombStore, HiveComb, HiveError, PlaceCombInput};

/// Emits a comb onto the hexagonal honeycomb lattice when an agent or subagent completes a task.
pub fn emit_subagent_comb(
    store: &CombStore,
    role: &str,
    summary: &str,
    parent_id: Option<&str>,
) -> Result<HiveComb, HiveError> {
    let author = format!("AIEN · {}", role);
    let input = PlaceCombInput {
        q: None,
        r: None,
        author,
        role: Some(role.to_string()),
        content: summary.to_string(),
        intent: Some("join".to_string()),
        parent_id: parent_id.map(|s| s.to_string()),
    };
    store.place_comb(input)
}

/// Emits a comb onto the hexagonal honeycomb lattice when Socratic inquiry is triggered.
pub fn emit_socratic_comb(
    store: &CombStore,
    question: &str,
    parent_id: Option<&str>,
) -> Result<HiveComb, HiveError> {
    let author = "AIEN (Socratic)".to_string();
    let input = PlaceCombInput {
        q: None,
        r: None,
        author,
        role: Some("socratic".to_string()),
        content: question.to_string(),
        intent: Some("branch".to_string()),
        parent_id: parent_id.map(|s| s.to_string()),
    };
    store.place_comb(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_and_socratic_emission() {
        let store = CombStore::open_in_memory().expect("in memory store");

        // 1. Emit socratic inquiry comb
        let socratic = emit_socratic_comb(
            &store,
            "What is the foundational invariant of this sovereign workspace?",
            Some("comb-genesis-00000000"),
        ).expect("socratic emission");

        assert_eq!(socratic.role, "socratic");
        assert_eq!(socratic.intent, "branch");
        assert_eq!(socratic.parent_id, Some("comb-genesis-00000000".to_string()));

        // 2. Emit subagent completed comb
        let sub = emit_subagent_comb(
            &store,
            "researcher",
            "Synthesized hexagonal honeycomb topology specifications.",
            Some(&socratic.id),
        ).expect("subagent emission");

        assert_eq!(sub.role, "researcher");
        assert_eq!(sub.intent, "join");
        assert_eq!(sub.parent_id, Some(socratic.id.clone()));

        // Verify total count on lattice
        let (cells, bounds) = store.get_cells().expect("get cells");
        assert_eq!(cells.len(), 3);
        assert_eq!(bounds.count, 3);
    }
}
