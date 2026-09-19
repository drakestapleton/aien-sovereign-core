"""Config subclass that expands Nemotron-H KV heads to satisfy MAX's GQA kernel."""

from __future__ import annotations

import math
from dataclasses import dataclass

from max.dtype import DType
from max.pipelines.architectures.nemotron_h.model_config import (
    NemotronHConfig,
    parse_hybrid_pattern,
    resolve_attention_head_dim,
)
from max.pipelines.lib.config.model_config import _select_quantization_encoding

MAX_NVIDIA_GQA_GROUP = 8


def kv_expansion_factor(huggingface_config) -> int:
    """How many times to replicate each KV head so group <= 8."""
    n_heads = int(huggingface_config.num_attention_heads)
    n_kv = int(huggingface_config.num_key_value_heads)
    if n_kv <= 0:
        return 1
    group = n_heads / n_kv
    return max(1, math.ceil(group / MAX_NVIDIA_GQA_GROUP))


@dataclass(kw_only=True)
class KvExpandedNemotronHConfig(NemotronHConfig):
    """NemotronHConfig with KV-head replication for the MAX MHA kernel."""

    kv_expansion: int = 1

    @staticmethod
    def construct_kv_params(
        huggingface_config,
        pipeline_config,
        devices,
        kv_cache_config,
        cache_dtype,
    ):
        # Mirror the built-in body, but multiply n_kv_heads by E so the cache,
        # the attention reshape, and the flash kernel all agree on group <= 8.
        expansion = kv_expansion_factor(huggingface_config)
        resolved_encoding = _select_quantization_encoding(
            pipeline_config.model, NemotronHConfig.DEFAULT_ENCODING
        )
        if (
            resolved_encoding == "float8_e4m3fn"
            and kv_cache_config.kv_cache_format is None
        ):
            cache_dtype = DType.float8_e4m3fn
        kinds = parse_hybrid_pattern(huggingface_config.hybrid_override_pattern)
        num_attention_layers = sum(1 for k in kinds if k == "attention")
        return kv_cache_config.to_params(
            dtype=cache_dtype,
            n_kv_heads=int(huggingface_config.num_key_value_heads) * expansion,
            head_dim=resolve_attention_head_dim(huggingface_config),
            num_layers=num_attention_layers,
            devices=devices,
            data_parallel_degree=pipeline_config.model.data_parallel_degree,
        )

    @classmethod
    def from_hf(
        cls,
        pipeline_config,
        huggingface_config,
        dtype,
        kv_params,
        devices,
    ):
        config = super().from_hf(
            pipeline_config, huggingface_config, dtype, kv_params, devices
        )
        expansion = kv_expansion_factor(huggingface_config)
        config.kv_expansion = expansion
        config.num_key_value_heads = (
            int(huggingface_config.num_key_value_heads) * expansion
        )
        return config
