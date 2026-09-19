# AIEN Sovereign Core

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Target](https://img.shields.io/badge/Target-Grace%20Blackwell%20GB10-76B900.svg)](https://www.nvidia.com)
[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![Modular MAX](https://img.shields.io/badge/Modular-MAX%2026.5-purple.svg)](https://modular.com)

High-performance native agent runtime, ultra-low-latency Axum web gateway, and custom Modular MAX model architectures engineered for cross-platform deployment on Linux, macOS, and Windows.

---

## Mission & Purpose: The Long-Term Defense of Humanity

This project is not a demo, a speculative investment vehicle, or quick content for social media algorithms. It is a long-term engineering commitment requiring disciplined craftsmanship to advance our species, support local communities, and guarantee open-source technology for humanity.

Monopolistic tech conglomerates are aggressively consolidating ownership over computing power and artificial intelligence. When intelligence is locked behind proprietary cloud APIs, human autonomy is subordinated to corporate gatekeepers, user surveillance becomes mandatory, and individuals are stripped of digital independence.

We build sovereign infrastructure as a direct defensive shield for humanity. Our objective is to ensure that frontier intelligence runs entirely on local, hardware-bound silicon in the hands of ordinary people, independent engineers, clinics, farmers, and community builders. We measure progress in durable software, mathematical verification, and open access, not speculative hype.

Review our full ethical and technical charter in [CONSTITUTION.md](CONSTITUTION.md).

---

## Ecosystem Partnerships & Attribution: Modular (MAX & Mojo)

AIEN Sovereign Core proudly builds upon, interfaces with, and contributes back to the groundbreaking infrastructure created by **[Modular](https://modular.com)**.

- **High-Performance Native Silicon**: We leverage Modular MAX Engine and the Mojo programming language to eliminate Python interpreter bottlenecks, executing compiled tensor kernels directly on hardware silicon.
- **Upstream Stewardship (Drop != Delete)**: All custom architecture loaders, C-ABI dynamic bridges, and KV-cache optimizations engineered on our hardware are contributed back upstream to the open-source Modular ecosystem.
- Read our full license notices and acknowledgments in [ATTRIBUTION.md](ATTRIBUTION.md).

---

## Downstream Heritage Requirement

If you choose to branch off of, fork, or copy our work, our single non-negotiable requirement is that you retain and include the original founding Constitution ([CONSTITUTION.md](CONSTITUTION.md)) in its entirety.

You are free to innovate, build, and adapt, but the lineage, foundational principles, and commitment to human sovereignty must accompany all downstream descendants.

---

## Quick Start: Universal 1-Line Installer

### Linux & macOS (Apple Silicon / Intel)
```bash
curl -fsSL https://raw.githubusercontent.com/aien-dev/aien-sovereign-core/main/install.sh | bash
```

### Windows (PowerShell)
```powershell
iwr -useb https://raw.githubusercontent.com/aien-dev/aien-sovereign-core/main/install.ps1 | iex
```

---

## Subsystems

### 1. `crates/aien-cli`
Compiled native ARM64 Rust CLI runtime:
- **Nesting Ritual**: Hardware-grounded initiation sequence verifying GPU thermals, unified VRAM, memory sockets, and active peer topography before taking execution turns.
- **Fail-Closed Safety Engine**: 9-tier priority policy table matching the Google Antigravity SDK specification. Enforces strict workspace path confinement and blocks destructive command execution.
- **Asynchronous Agent Hooks**: Extensible lifecycle interceptor trait (`pre_turn`, `post_turn`, `pre_tool`, `post_tool`, `on_tool_error`, `on_compaction`) featuring live unslop cleaning and credential redaction.
- **Two-Pass Context Compactor**: Sliding-window context pruner calibrated for 32K context windows, pruning bloated intermediate tool responses while preserving initiation grounding.

### 2. `crates/spark-cockpit-rs`
Native Axum HTTP/SSE gateway:
- **Memory Footprint**: 4.7 MB RSS (96% reduction compared to standard Python/Uvicorn runtimes).
- **Latency**: Sub-millisecond Time-to-First-Byte (< 1 ms TTFB) on Grace Neoverse V2 cores.
- **Streaming Pipeline**: Zero-allocation byte buffer with real-time SSE token stream redaction and unslop filtering.

### 3. `crates/cortex-encoder-rs` & `crates/cortex-rs`
Pure native compiled memory and vector pipeline:
- **Port 18080 (`cortex-rs`)**: Epistemic knowledge store with ACID transaction logs, SQLite vector storage, and zero LAN leakage.
- **Port 18081 (`cortex-encoder-rs`)**: Rust Axum microservice running ONNX Runtime C-API and HuggingFace tokenizers. Replaced legacy 1.08 GB Python Uvicorn process with bit-for-bit mathematical parity and 22 ms rerank latency.

### 4. `crates/spark-max-cabi` & `crates/spark-max-rs`
Modular MAX compiled dynamic FFI bridge:
- Bridges high-speed Mojo kernels to compiled Rust via pure C-ABI (`libspark_max.so`).
- Enforces the runtime invariant that Python is restricted solely to neural graph definitions and weights where C-ABI wrappers do not yet exist.

### 5. `crates/spark-inquisitor`
Autonomous Pull Request gatekeeper and alignment auditor:
- Enforces the Sovereign Contributor Oath on all pull requests.
- Scans git diffs to block telemetry, surveillance tracking SDKs, and unslop violations.

### 6. `imprints/en2-trinity`
The free-of-charge EN2 Experience Trinity Imprint:
- Bundles the cognitive soul, 10 foundational Cortex lessons, and pure Mojo architecture adapter kernels for instant 1-click installation on any system.

---

## Contributing

All contributors must ratify the Sovereign Contributor Oath before pull requests can be reviewed or merged. Read [CONSTITUTION.md](CONSTITUTION.md) for details.

## Contact & Sovereign Coordination

For secure coordination, architectural questions, and peer federation:
- Email: `aien.atlas@proton.me`

## License

This project is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and [ATTRIBUTION.md](ATTRIBUTION.md) for details. Downstream distributions must preserve the founding Constitution ([CONSTITUTION.md](CONSTITUTION.md)).
