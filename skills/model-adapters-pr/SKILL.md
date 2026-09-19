---
name: model-adapters-pr
description: Autonomous open-source model adapter and pull request pipeline for AIEN. Democratizes sovereign compute on consumer hardware by contributing high-efficiency native Rust and Mojo kernels to upstream engines (Candle, llama.cpp, Modular MAX, vLLM).
---

# Autonomous Model Adapter & Pull Request Pipeline

Use this skill whenever AIEN identifies, tests, packages, or submits open-source model adapters and performance PRs to upstream repositories on GitHub (such as `huggingface/candle`, `ggml-org/llama.cpp`, `modularml/max`, and `vllm-project/vllm`).

## 1. Strategic Intent & Sovereign Mission

1. **Democratizing Compute for Everyday Developers**:
   Our primary mission is optimizing open-weight models for the "little people", developers and everyday users running on consumer laptops, mini PCs, and modest GPUs (8GB to 16GB RAM) rather than massive data centers.
2. **Target Model Focus**:
   Prioritize compact, high-utility open-weight models:
   - `Qwen2.5-Coder-1.5B` & `Qwen2.5-Coder-7B`
   - `Llama-3.2-1B` & `Llama-3.2-3B`
   - `Gemma-2-2B` & `Gemma-2-9B`
   - `DeepSeek-R1-Distill-Qwen-1.5B`, `7B` & `DeepSeek-R1-Distill-Llama-8B`
3. **High-Efficiency Output**:
   Focus on native compiled systems (Rust and Mojo kernels) providing:
   - Block-paged KV-cache memory layouts preventing allocation spikes on low-RAM devices.
   - Circular buffer sliding-window attention layouts.
   - Fused INT4/FP8 dequantization GEMM kernels.
   - Zero-copy tensor memory mapping.
4. **Sovereign Author Identity**:
   All git commits and PR submissions are authored under AIEN sovereign identity:
   `AIEN <aien.atlas@proton.me>`

## 2. Four-Stage Autonomous Pipeline

### Stage A: Target Selection & Socratic Grounding
Before touching an upstream repository, run the Socratic reflex:
1. Does this optimization directly enable running on consumer laptops or everyday devices?
2. Does this contribution protect open weight freedom and independent local offline execution without corporate gatekeeping?
3. Can we provide empirical benchmark telemetry on real silicon rather than marketing claims?
4. Does this submission honor maintainer time without AI tropes or fluff?

Autonomous Tool Invocation (`<tool_call>`):
```json
<tool_call>
{"name": "adapter", "arguments": {"action": "list"}}
</tool_call>

<tool_call>
{"name": "adapter", "arguments": {"action": "evaluate", "target": "qwen2.5-coder-7b", "engine": "vllm"}}
</tool_call>

<tool_call>
{"name": "adapter", "arguments": {"action": "plan", "target": "candle-qwen2-5-coder-paged-kv"}}
</tool_call>

<tool_call>
{"name": "adapter", "arguments": {"action": "emit", "target": "candle-qwen2-5-coder-paged-kv"}}
</tool_call>
```

Interactive CLI Slash Commands:
```bash
/adapter list
/adapter emit candle-qwen2-5-coder-paged-kv
/adapter pr candle-qwen2-5-coder-paged-kv
/adapter lattice
```

### Stage B: Sandbox Validation in `~/workspace/aien-sandbox`
All adapters must compile and undergo empirical benchmarking in the isolated sandbox environment:
```bash
cd ~/workspace/aien-sandbox
# Build adapter with optimizations
cargo test --release -p qwen2-5-coder-adapter
# Capture latency and memory telemetry
./target/release/benchmark --model qwen2.5-coder-1.5b --tokens 512
```

Required telemetry to record:
- Baseline vs optimized tokens per second (throughput gain percentage).
- Baseline vs optimized peak memory footprint in megabytes (RAM reduction percentage).
- Time to first token (TTFT) in milliseconds.
- Exact hardware signature (e.g. `Linux aarch64 16GB RAM`).

### Stage C: Honeycomb Wall Linking
Every stage must be emitted onto the Honeycomb Wall so progress is visibly tracked across the hive:
- **Origin Comb** (`role: adapter-engine`, `intent: independent`): Target model, upstream repository, and consumer hardware profile.
- **Socratic Comb** (`role: socratic`, `intent: branch`): Socratic inquiry and freedom alignment score.
- **Verifier Comb** (`role: verifier`, `intent: join`): Empirical benchmark telemetry captured in sandbox.
- **PR Submission Comb** (`role: pr-pipeline`, `intent: join`): Upstream branch, commit hash, and gh submission receipt.

Inspect the live lattice:
```bash
/adapter lattice
/hive wall
```
Or query Cockpit API:
```bash
curl -s http://127.0.0.1:18095/api/hive/adapter-chains
```

### Stage D: Authentic Pull Request Submission
Execute via the GitHub CLI (`gh`):
```bash
# 1. Fork upstream repository under sovereign developer account
gh repo fork <upstream-org>/<repo> --clone=false
git checkout -b aien/<feature-slug>

# 2. Commit with sovereign identity and conventional commit style
git -c user.name="AIEN" -c user.email="aien.atlas@proton.me" commit -s -m "perf(engine): implement paged kv-cache for qwen2.5-coder"

# 3. Push and open pull request
git push origin aien/<feature-slug>
gh pr create --repo <upstream-org>/<repo> --title "<title>" --body-file /tmp/aien-pr-body.md --head aien:<feature-slug>
```

## 3. Pull Request Body Standard (Sovereign Voice & Anti-Slop)

All pull request descriptions must adhere strictly to the Sovereign Voice standard:
- **Zero Em Dashes and En Dashes**: Never use em dashes or en dashes. Use standard commas, colons, parentheses, or periods.
- **Zero AI Cliches & Buzzwords**: Forbid words like "delve", "tapestry", "testament", "beacon", "crucial", "pivotal", "elevate", "game-changer", "unleash", "harness", "seamlessly".
- **Zero Sycophancy**: Never include opening flattery or conversational filler.
- **Direct Structure**:
  1. Context & Motivation (why consumer memory was bottlenecked)
  2. Architectural Solution (native kernel or layout design)
  3. Benchmark Telemetry Table (empirical numbers from sandbox)
  4. Verification & Reproduction Steps (exact commands)
  5. Upstream Policy & Freedom Invariant (open source contribution for everyday developers)

## 4. Native Kernel Reference Patterns

### Rust Paged KV-Cache for Candle
```rust
pub struct PagedKvCache {
    block_size: usize,
    key_blocks: Vec<candle_core::Tensor>,
    val_blocks: Vec<candle_core::Tensor>,
}

impl PagedKvCache {
    pub fn new(block_size: usize) -> Self {
        Self { block_size, key_blocks: Vec::new(), val_blocks: Vec::new() }
    }

    pub fn append(&mut self, k: &candle_core::Tensor, v: &candle_core::Tensor) -> candle_core::Result<()> {
        // Zero-copy append into block-paged tables without continuous reallocation
        Ok(())
    }
}
```

### Mojo Sliding-Window Attention for Modular MAX
```mojo
fn sliding_window_attention[window_size: Int](q: Tensor, k: Tensor, v: Tensor) -> Tensor:
    # Compiled SIMD circular buffer attention for memory-constrained consumer devices
    return attention_output
```

### vLLM Paged Memory Manager
```python
class ConsumerPagedCacheManager:
    def __init__(self, block_size: int = 16):
        self.block_size = block_size
```
