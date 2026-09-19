"""Weight adapter: built-in Nemotron-H adapter, + optional PEFT LoRA fold-in,
then KV-head replication for MAX's GQA kernel (group 16 -> 8).

Track A: set ``MAX_LORA_DIR`` to a PEFT adapter directory (adapter_config.json +
adapter_model.safetensors). Its q/k/v/o LoRA weights are folded into the base
projections *before* qkv fusion, using delta = (alpha/r) * B @ A. This lets an
adapter trained with NeMo/PEFT be served on MAX for NemotronH without MAX's
Llama-3-only runtime LoRA path. The adapter is baked at load time.
"""

from __future__ import annotations

import json
import os
import re

import numpy as np
from max.driver import Buffer
from max.dtype import DType
from max.graph.weights import WeightData
from max.graph.weights.weights import Shape
from max.pipelines.architectures.nemotron_h.model_config import (
    resolve_attention_head_dim,
)
from max.pipelines.architectures.nemotron_h.weight_adapters import (
    convert_nemotron_h_state_dict,
)

from .config import kv_expansion_factor

_LORA_PROJ_RE = re.compile(r"layers\.(\d+)\.mixer\.(q|k|v|o)_proj\.")
_FUSED_RE = re.compile(r"^blocks\.(\d+)\.mixer\.qkv_proj\.weight$")


def _to_numpy_u16or(dtype: DType, buf: Buffer) -> np.ndarray:
    if dtype == DType.bfloat16:
        return np.from_dlpack(Buffer.from_dlpack(buf).view(DType.uint16))
    return np.from_dlpack(buf)


def _bf16_to_f32(u16: np.ndarray) -> np.ndarray:
    return (u16.astype(np.uint32) << 16).view(np.float32)


def _f32_to_bf16(x: np.ndarray) -> np.ndarray:
    return (x.view(np.uint32) >> 16).astype(np.uint16)


def _load_lora(lora_dir: str) -> tuple[dict, float, int]:
    from safetensors.numpy import load_file

    cfg = json.loads(open(os.path.join(lora_dir, "adapter_config.json")).read())
    r = int(cfg.get("r", 0))
    alpha = float(cfg.get("lora_alpha", r))
    scale = (alpha / r) if r else 1.0
    sd = load_file(os.path.join(lora_dir, "adapter_model.safetensors"))
    by: dict[tuple[int, str], dict[str, np.ndarray]] = {}
    for name, value in sd.items():
        m = _LORA_PROJ_RE.search(name)
        if not m:
            continue
        layer, proj = int(m.group(1)), m.group(2)
        if name.endswith("lora_A.weight"):
            by.setdefault((layer, proj), {})["A"] = value
        elif name.endswith("lora_B.weight"):
            by.setdefault((layer, proj), {})["B"] = value
    return by, scale, r


def _delta(a: np.ndarray, b: np.ndarray, scale: float) -> np.ndarray:
    return (scale * (b.astype(np.float32) @ a.astype(np.float32))).astype(
        np.float32
    )


def _apply_lora_fused(
    wd: WeightData,
    lora: dict[str, np.ndarray],
    scale: float,
    q_dim: int,
    kv_dim: int,
) -> WeightData:
    """q/k/v deltas folded into the fused qkv weight (rows: q|k|v)."""
    hidden = int(wd.shape[1])
    dtype = wd.dtype
    data = _to_numpy_u16or(dtype, wd.data)
    w32 = _bf16_to_f32(data) if dtype == DType.bfloat16 else data.astype(np.float32)
    q = w32[:q_dim]
    k = w32[q_dim : q_dim + kv_dim]
    v = w32[q_dim + kv_dim :]
    if "A" in lora.get("q", {}) :
        q = q + _delta(lora["q"]["A"], lora["q"]["B"], scale)
    if "A" in lora.get("k", {}):
        k = k + _delta(lora["k"]["A"], lora["k"]["B"], scale)
    if "A" in lora.get("v", {}):
        v = v + _delta(lora["v"]["A"], lora["v"]["B"], scale)
    fused = np.ascontiguousarray(np.concatenate([q, k, v], axis=0))
    out_rows = q_dim + 2 * kv_dim
    if dtype == DType.bfloat16:
        buf = Buffer.from_dlpack(_f32_to_bf16(fused)).view(
            dtype=DType.uint16, shape=(out_rows, hidden)
        )
        # keep the bf16 view for the engine
        buf = buf.view(dtype=DType.bfloat16, shape=(out_rows, hidden))
    else:
        buf = Buffer.from_dlpack(fused).view(dtype=dtype, shape=(out_rows, hidden))
    return WeightData(
        data=buf,
        name=wd.name,
        dtype=dtype,
        shape=Shape([out_rows, hidden]),
        quantization_encoding=wd.quantization_encoding,
    )


def _expand_fused_kv(
    wd: WeightData, *, n_heads: int, n_kv: int, head_dim: int, expansion: int
) -> WeightData:
    q_dim = n_heads * head_dim
    kv_dim = n_kv * head_dim
    hidden = int(wd.shape[1])
    rows = int(wd.shape[0])
    if rows != q_dim + 2 * kv_dim:
        raise ValueError(f"unexpected fused qkv rows {rows}; expected {q_dim + 2 * kv_dim}")
    data = _to_numpy_u16or(wd.dtype, wd.data)
    q = data[:q_dim]
    k = data[q_dim : q_dim + kv_dim]
    v = data[q_dim + kv_dim :]

    def rep(block: np.ndarray) -> np.ndarray:
        block = block.reshape(n_kv, head_dim, hidden)
        block = np.repeat(block, expansion, axis=0)
        return block.reshape(n_kv * expansion * head_dim, hidden)

    new_kv_dim = kv_dim * expansion
    fused = np.ascontiguousarray(np.concatenate([q, rep(k), rep(v)], axis=0))
    out_rows = q_dim + 2 * new_kv_dim
    buf = Buffer.from_dlpack(fused).view(dtype=wd.dtype, shape=(out_rows, hidden))
    return WeightData(
        data=buf,
        name=wd.name,
        dtype=wd.dtype,
        shape=Shape([out_rows, hidden]),
        quantization_encoding=wd.quantization_encoding,
    )


def convert_kvexp_nemotron_h_state_dict(
    state_dict, huggingface_config, pipeline_config, **unused_kwargs
):
    base = convert_nemotron_h_state_dict(
        state_dict, huggingface_config, pipeline_config, **unused_kwargs
    )

    n_heads = int(huggingface_config.num_attention_heads)
    n_kv = int(huggingface_config.num_key_value_heads)
    head_dim = resolve_attention_head_dim(huggingface_config)
    q_dim = n_heads * head_dim
    kv_dim = n_kv * head_dim

    # --- Track A: optional PEFT LoRA fold-in (before KV expansion) ---
    lora_dir = os.environ.get("MAX_LORA_DIR") or os.environ.get("ATLAS_LORA_DIR")
    if lora_dir:
        by, scale, rank = _load_lora(lora_dir)
        msg = (
            f"[lora] folding adapter {lora_dir} (r={rank}, scale={scale}) "
            f"into {len({k[0] for k in by})} attention layers"
        )
        print(msg, flush=True)
        try:
            with open(os.path.join(lora_dir, ".folded"), "a") as _f:
                import time as _t

                _f.write(f"{_t.time()} {msg}\n")
        except OSError:
            pass
        for name, wd in list(base.items()):
            m = _FUSED_RE.match(name)
            if not m:
                continue
            layer = int(m.group(1))
            per_proj = {p: by.get((layer, p), {}) for p in ("q", "k", "v")}
            base[name] = _apply_lora_fused(wd, per_proj, scale, q_dim, kv_dim)
        # o_proj deltas
        for (layer, proj), ab in by.items():
            if proj != "o" or "A" not in ab:
                continue
            oname = f"blocks.{layer}.mixer.o_proj.weight"
            wd = base.get(oname)
            if wd is None:
                continue
            if wd.dtype == DType.bfloat16:
                w32 = _bf16_to_f32(_to_numpy_u16or(wd.dtype, wd.data))
            else:
                w32 = _to_numpy_u16or(wd.dtype, wd.data).astype(np.float32)
            w32 = w32 + _delta(ab["A"], ab["B"], scale)
            if wd.dtype == DType.bfloat16:
                buf = Buffer.from_dlpack(_f32_to_bf16(w32)).view(DType.uint16)
                buf = buf.view(dtype=DType.bfloat16, shape=wd.shape)
            else:
                buf = Buffer.from_dlpack(w32).view(dtype=wd.dtype, shape=wd.shape)
            base[oname] = WeightData(
                data=buf, name=wd.name, dtype=wd.dtype, shape=wd.shape,
                quantization_encoding=wd.quantization_encoding,
            )

    # --- KV-head replication for the MAX Mojo GQA kernel (group <= 8) ---
    expansion = kv_expansion_factor(huggingface_config)
    if expansion == 1:
        return base
    for name, wd in list(base.items()):
        if name.endswith(".mixer.qkv_proj.weight"):
            base[name] = _expand_fused_kv(
                wd, n_heads=n_heads, n_kv=n_kv, head_dim=head_dim, expansion=expansion
            )
    return base