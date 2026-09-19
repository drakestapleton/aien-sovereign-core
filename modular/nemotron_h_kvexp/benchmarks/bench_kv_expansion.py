"""Benchmark KV head replication latency and memory throughput on ARM64."""

import time
import numpy as np


def benchmark_kv_replication():
    # Nemotron-30B attention layer dimensions
    n_heads = 32
    head_dim = 128
    n_kv = 2
    expansion = 2
    hidden = 4096
    num_attention_layers = 52

    q_dim = n_heads * head_dim      # 4096
    kv_dim = n_kv * head_dim        # 256
    total_rows = q_dim + 2 * kv_dim # 4608

    # Synthetic fused QKV matrix in FP32
    qkv = np.random.randn(total_rows, hidden).astype(np.float32)

    # Warmup
    for _ in range(5):
        q = qkv[:q_dim]
        k = qkv[q_dim : q_dim + kv_dim].reshape(n_kv, head_dim, hidden)
        v = qkv[q_dim + kv_dim :].reshape(n_kv, head_dim, hidden)
        k_rep = np.repeat(k, expansion, axis=0).reshape(n_kv * expansion * head_dim, hidden)
        v_rep = np.repeat(v, expansion, axis=0).reshape(n_kv * expansion * head_dim, hidden)
        fused = np.ascontiguousarray(np.concatenate([q, k_rep, v_rep], axis=0))

    # Benchmark single layer
    iterations = 200
    start = time.perf_counter()
    for _ in range(iterations):
        q = qkv[:q_dim]
        k = qkv[q_dim : q_dim + kv_dim].reshape(n_kv, head_dim, hidden)
        v = qkv[q_dim + kv_dim :].reshape(n_kv, head_dim, hidden)
        k_rep = np.repeat(k, expansion, axis=0).reshape(n_kv * expansion * head_dim, hidden)
        v_rep = np.repeat(v, expansion, axis=0).reshape(n_kv * expansion * head_dim, hidden)
        fused = np.ascontiguousarray(np.concatenate([q, k_rep, v_rep], axis=0))
    elapsed = time.perf_counter() - start

    per_layer_ms = (elapsed / iterations) * 1000.0
    full_model_ms = per_layer_ms * num_attention_layers
    bytes_processed = qkv.nbytes * iterations
    throughput_gb_s = (bytes_processed / (1024**3)) / elapsed

    print("=== Nemotron-H KV Expansion Benchmark (Grace Neoverse V2) ===")
    print(f"Layer Shape: [{total_rows}, {hidden}] -> [{q_dim + 2 * n_kv * expansion * head_dim}, {hidden}]")
    print(f"Latency per Layer:      {per_layer_ms:.3f} ms")
    print(f"Full 52-Layer Model:    {full_model_ms:.2f} ms")
    print(f"Memory Throughput:      {throughput_gb_s:.2f} GB/s")
    print("=============================================================")


if __name__ == "__main__":
    benchmark_kv_replication()
