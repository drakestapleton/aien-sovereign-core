pub mod geometry;
pub mod store;
pub mod emission;
pub mod adapter_engine;

pub use geometry::{
    axial_to_pixel, hex_distance, neighbors, pixel_to_axial, round_axial,
    DetailTier, HexCoord, PlacementIntent, DEFAULT_HEX_RADIUS, HEX_DIRECTIONS,
    HEX_DIRECTION_LABELS, SQRT_3,
};
pub use store::{BoundingBox, CombStore, CreateForgeTaskInput, ForgeTask, HiveComb, HiveError, NeighborLink, PlaceCombInput};
pub use emission::{emit_socratic_comb, emit_subagent_comb};
pub use adapter_engine::{
    evaluate_socratic_reflex, generate_pr_plan, emit_adapter_pipeline_combs,
    list_adapter_pipeline_chains, get_catalog_adapters, build_adapter_spec, find_or_create_adapter,
    ConsumerModel, HardwareProfile, UpstreamEngine, AdapterCategory,
    SocraticEvaluation, SocraticInquiry, BenchmarkTelemetry, AdapterSpec,
    PrSubmissionPlan, AdapterPipelineReceipt, AdapterPipelineChain,
};
