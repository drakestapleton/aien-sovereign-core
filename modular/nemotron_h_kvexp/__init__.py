"""KV-head-expanded Nemotron-H adapter for MAX (see config.py for rationale)."""

from .arch import kvexp_arch
from .model import KvExpandedNemotronHModel

ARCHITECTURES = [kvexp_arch]

# MAX registers built-in architectures *lazily*: ``register_lazy(name, ...)``
# records a pending entry, and the first lookup calls ``_materialize_lazy`` ->
# ``register(builtin)`` WITHOUT ``allow_override``. Since our arch already
# registered under the same name, that raises:
#   "Refusing to override existing architecture for 'NemotronHForCausalLM'".
# We are imported before materialization, so wrap ``register`` to ignore the
# built-in re-registration for this name and keep ours.
try:
    from max.pipelines import PIPELINE_REGISTRY

    _orig_register = PIPELINE_REGISTRY.register

    def _register_kvexp_aware(architecture, *args, **kwargs):
        if (
            architecture.name == "NemotronHForCausalLM"
            and architecture.pipeline_model is not KvExpandedNemotronHModel
            and (architecture.name, architecture.task) in PIPELINE_REGISTRY._architectures_by_task
        ):
            # Built-in lazily materializing after our override; keep ours.
            return
        return _orig_register(architecture, *args, **kwargs)

    PIPELINE_REGISTRY.register = _register_kvexp_aware
except Exception:
    pass
