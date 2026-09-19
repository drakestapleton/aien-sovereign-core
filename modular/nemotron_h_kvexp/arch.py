"""Register the KV-expanded Nemotron-H architecture (overrides the built-in)."""

from __future__ import annotations

from max.graph.weights import WeightsFormat
from max.pipelines.context import TextContext
from max.pipelines.lib import SupportedArchitecture
from max.pipelines.modeling.types import PipelineTask

# Nemotron-3's chat template is Qwen-format; importing the qwen3_5 parsers here
# registers the "qwen3_5" reasoning/tool parsers the architecture names below.
from max.pipelines.architectures.qwen3_5.reasoning import (  # noqa: F401
    Qwen3_5ReasoningParser,
)
from max.pipelines.architectures.qwen3_5.tool_parser import (  # noqa: F401
    Qwen3_5ToolParser,
)
from max.pipelines.architectures.nemotron_h.tokenizer import NemotronHTokenizer

from .config import KvExpandedNemotronHConfig
from .model import KvExpandedNemotronHModel
from .weight_adapters import convert_kvexp_nemotron_h_state_dict

kvexp_arch = SupportedArchitecture(
    # Same name as the built-in -> allowed to override so the checkpoint's
    # config.json architectures[0] == "NemotronHForCausalLM" resolves here.
    name="NemotronHForCausalLM",
    task=PipelineTask.TEXT_GENERATION,
    example_repo_ids=[
        "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16",
    ],
    default_weights_format=WeightsFormat.safetensors,
    default_encoding=KvExpandedNemotronHConfig.DEFAULT_ENCODING,
    supported_encodings=KvExpandedNemotronHConfig.SUPPORTED_ENCODINGS,
    pipeline_model=KvExpandedNemotronHModel,
    tokenizer=NemotronHTokenizer,
    context_type=TextContext,
    weight_adapters={
        WeightsFormat.safetensors: convert_kvexp_nemotron_h_state_dict,
    },
    # SSM recurrent state is not reconstructable from a token prefix.
    required_arguments={"enable_prefix_caching": False},
    config=KvExpandedNemotronHConfig,
    multi_gpu_supported=False,
    reasoning_parser="qwen3_5",
    tool_parser="qwen3_5",
    supports_device_graph_capture=True,
)
