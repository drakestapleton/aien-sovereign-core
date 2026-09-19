# KV-Head Expanded Nemotron-H Architecture for Modular MAX

Custom architecture adapter enabling high-throughput serving of NVIDIA Nemotron-H models (such as `nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16`) on Modular MAX.

## Architecture Rationale

Modular MAX employs optimized FlashAttention kernels for Grouped-Query Attention (GQA). In current MAX versions, the maximum supported GQA group size is 8 (`group = num_attention_heads / num_key_value_heads <= 8`).

Nemotron-3.5-Lightning-30B utilizes a GQA configuration where:
- Query heads (`n_heads`): 32
- Key-Value heads (`n_kv`): 2
- GQA group ratio: 32 / 2 = 16

Because 16 > 8, loading the standard checkpoint triggers an unsupported group size error in the MAX attention kernel.

This adapter resolves the kernel limitation by calculating the minimum integer expansion factor:

```python
expansion = math.ceil((n_heads / n_kv) / MAX_NVIDIA_GQA_GROUP)
```

For Nemotron-H (group 16), `expansion = 2`. Each KV head is replicated across the channel dimension during state dictionary weight loading:
- Replicated KV heads: 2 * 2 = 4
- Effective GQA group ratio: 32 / 4 = 8

This satisfies the FlashAttention kernel constraint without altering model activations, attention math, or output distributions.

## Key Features

1. **Native Dynamic Expansion**: Replicates KV heads on the fly during safe tensor loading and state dict conversion.
2. **Offline PEFT LoRA Folding**: Set `MAX_LORA_DIR` to fold LoRA adapters (A and B matrices) directly into base weights prior to QKV fusion, eliminating runtime LoRA overhead.
3. **Lazy Registration Override**: Overrides default `NemotronHForCausalLM` registration within the MAX pipeline registry to ensure transparent resolution from standard model checkpoints.

## Installation & Usage

Install as an editable package:

```bash
pip install -e .
```

Serve using the MAX CLI:

```bash
max serve \
  --model nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 \
  --served-model-name nemotron-lightning \
  --custom-architectures $(pwd) \
  --port 18006 \
  --max-batch-size 8 \
  --max-length 32768 \
  --kv-cache-format bfloat16
```

## Testing

Run unit tests via standard Python unittest:

```bash
python3 -m unittest discover -s tests -v
```

## License

Licensed under the Apache License, Version 2.0.

## Benchmarks (NVIDIA DGX Spark / Grace Blackwell GB10)

Benchmarked on ARM64 Grace Neoverse V2 cores using realistic Nemotron-30B layer dimensions (4608x4096 expanded to 5120x4096):

| Metric | Measured Value |
|---|---|
| Latency per Layer | 5.49 ms |
| Full Model Expansion (52 layers) | 285.63 ms |
| Memory Copy Throughput | 12.80 GB/s |

Load-time expansion overhead is under 300 ms for the entire 30B model, introducing negligible startup delay while enabling high-throughput serving on Modular MAX.
