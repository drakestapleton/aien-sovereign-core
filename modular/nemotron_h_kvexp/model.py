"""Pipeline model subclass that uses the KV-expanded config."""

from __future__ import annotations

from typing import Any

from max.graph import DeviceRef
from max.pipelines.architectures.nemotron_h.model import NemotronHModel

from .config import KvExpandedNemotronHConfig


class KvExpandedNemotronHModel(NemotronHModel):
    """NemotronHModel whose config replicates KV heads (GQA group <= 8).

    The built-in ``_create_model_config`` hardcodes ``NemotronHConfig.from_hf``,
    so we must override it to route through the expanded config. Everything else
    (graph build, state pools, batching) is inherited unchanged.
    """

    model_config_cls = KvExpandedNemotronHConfig

    def _create_model_config(self, state_dict: dict[str, Any]):
        device_ref = DeviceRef.from_device(self.devices[0])
        model_config = KvExpandedNemotronHConfig.from_hf(
            self.pipeline_config,
            self.huggingface_config,
            self.dtype,
            self.kv_params,
            [device_ref],
        )
        model_config.populate_fp8_layers(state_dict)
        return model_config
