# EN2 Sovereign Modular MAX Architecture Adapter Kernel
# Pure compiled Mojo dynamic library for low-latency inference on Grace Blackwell GB10

@export("en2_adapter_version")
def en2_adapter_version() -> Int32:
    return 2

@export("en2_kv_expand_stride")
def en2_kv_expand_stride(head_dim: Int32, num_heads: Int32) -> Int32:
    if head_dim <= 0 or num_heads <= 0:
        return -1
    return head_dim * num_heads * 2

@export("en2_apply_kv_rope_scaling")
def en2_apply_kv_rope_scaling(seq_len: Int32, base_freq: Float32) -> Float32:
    if seq_len <= 4096:
        return base_freq
    return base_freq * (1.0 + 0.1 * (Float32(seq_len) / 4096.0))
