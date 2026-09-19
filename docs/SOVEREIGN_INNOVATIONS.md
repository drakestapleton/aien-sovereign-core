# Sovereign Core Architectural Innovations

### Monorepo Blueprint of 17 Compiled Native Crates, Invariant Gates, and Local Hardware Acceleration

`aien-sovereign-core` is the foundational monorepo of the Sovereign AI Commons. Engineered in pure native compiled Rust and Mojo 1.1, it provides the operating system substrate for autonomous agents, local GPU inference, and collective defense on NVIDIA DGX Spark workstations and Grace Blackwell GB10 hardware.

---

## 1. Monorepo Crate Directory

The workspace comprises 17 native compiled crates, each dedicated to an isolated system responsibility:

### Governance and Verification
- **`crates/spark-inquisitor`**: Autonomous GitHub gatekeeper, diff auditor, and issue triage engine. Enforces constitutional invariants on pull requests and issues, audits diffs for tracking patterns, verifies unslop compliance, and evaluates contributor oaths.
- **`crates/spark-eval`**: Autonomous evaluation system providing JSON Schema validation, regression tracking, and execution reliability verification for agent actions.
- **`crates/spark-debugger`**: Hardware-level crash diagnostics and GPU fault isolation engine, inspecting execution traces and PTX/CUDA faults.

### Memory and Epistemics
- **`crates/cortex-rs`**: Persistent epistemic memory engine backed by SQLite WAL with FTS5 lexical full-text indexing, vector similarity, and entity provenance tracking.
- **`crates/cortex-encoder-rs`**: Native Rust embedding encoder providing fast local text vectorization for semantic retrieval.
- **`crates/spark-dream`**: Dynamic dream cycle engine that activates during GPU idle cycles to consolidate episodic session logs into verified semantic knowledge entities.
- **`crates/spark-crumbs`**: Cryptographic, append-only operational event ledger implementing the Crumb protocol with session trees and stigmergic directory anchors.

### Process Lifecycle and Orchestration
- **`crates/spark-supervisor`**: High-reliability process supervisor, watchdog, and crash recovery daemon managing agent processes with deterministic restart policies.
- **`crates/spark-hive`**: Honeycomb hive engine with 2D axial hexagonal geometry for routing agent tasks and coordinating consumer model adapters.
- **`crates/spark-cockpit-rs`**: Real-time terminal user interface (TUI) providing live visualization of memory topologies, GPU load, and agent dialogue trees.
- **`crates/aien-cli`**: Sovereign command-line interface and system orchestrator.

### Hardware Acceleration and Model Serving
- **`crates/spark-max-cabi` & `crates/spark-max-rs`**: Direct C-ABI foreign function bridge to Modular MAX, enabling low-latency model inference on Grace Blackwell GB10 without Python runtime dependencies.
- **`crates/spark-adapters`**: Consumer model adapter pipeline enabling quantization, weight formatting, and hardware adaptation.
- **`crates/spark-harvester`**: High-throughput reasoning trace extractor and dataset distillation engine.
- **`crates/spark-mail-rs`**: Sovereign peer-to-peer messaging and notification subsystem.

---

## 2. Constitutional Invariant Gates

Every crate and workflow within `aien-sovereign-core` adheres to four architectural axioms:

1. **Zero Telemetry and Anti-Surveillance**:
   All crates are prohibited from calling external telemetry, surveillance, or analytics endpoints. Network activity is restricted strictly to user-configured endpoints and peer relays.
2. **Pure Compiled Native Systems**:
   All daemons, memory engines, gateways, and CLI tools must compile to native machine binaries (Rust and Mojo). Interpreted runtimes are banned from core services.
3. **Hardware TPM Key Vault (Zero Disk Secrets)**:
   Credentials, API keys, and signing tokens never touch disk in plaintext. All secrets resolve dynamically in memory from the hardware TPM vault (`atlas-vault`) with automated stream redaction (`[REDACTED_BY_ATLAS_VAULT]`).
4. **Sovereign Voice and Anti-Slop**:
   All code comments, CLI outputs, and documentation strictly ban em dashes, en dashes, and formulaic AI marketing buzzwords.

---

## 3. Hardware Targets and Benchmark Proof

Verified on NVIDIA Grace Blackwell GB10 (NVIDIA DGX Spark, Linux aarch64):
- **Core Binary Build Time**: < 1.0s release compilation for `spark-inquisitor`.
- **Memory Consumption**: Core background daemons operate under 25 MB resident memory each.
- **Inference Concurrency**: Sustained local generation hitting 94% to 96% GPU utilization at 20.7 tokens per second on local Modular MAX endpoints.

---

## 4. License and Commons Covenant

Licensed under the **Sovereign Resource Commons License 1.0 (SRCL-1.0)**.
Copyright (c) 2026 Drake Stapleton <drake.aien@proton.me> & AIEN <aien.atlas@proton.me>.
See [LICENSE](LICENSE) for terms.
For encrypted sovereign coordination: Drake Stapleton (`drake.aien@proton.me`) and AIEN (`aien.atlas@proton.me`).
