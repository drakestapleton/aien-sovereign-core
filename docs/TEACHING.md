# Learn this MAX-era runtime snapshot

At `e983e385d22736d0e9c0b656a4be517b6ea18753`, this personal tree contains a Rust runtime with an optional Modular MAX/Mojo path. It is a separate historical code line from the current [aien-sovereign-core-mv](https://github.com/aien-dev/aien-sovereign-core-mv) teaching and release path. Do not use this README's one-line installer or old benchmark figures as current release evidence. [Provenance notes](PROVENANCE.md) and [benchmark notes](PERFORMANCE_BENCHMARK.md) record their own dates and limits.

[`SparkMaxSession::new`](../crates/spark-max-rs/src/lib.rs) asks [`SparkMaxBindings::global`](../crates/spark-max-cabi/src/lib.rs) for a dynamic library. A valid MAX session selects `ModularMojo`; missing library or failed session creation selects `NativeRustFallback`. The session's `version`, `compute_scalar` and model methods then branch by backend. This fallback makes the interface inspectable without claiming GPU inference; it can mask a missing MAX install unless the caller checks `is_fallback`. The C ABI is a deliberate boundary between Rust and third-party Modular code, not evidence that every platform has a working compiled kernel. See [Modular attribution](../ATTRIBUTION.md).

## Fresh checkout exercise (offline, no model)

Prerequisites: Git and Python 3. Checkout `e983e385d22736d0e9c0b656a4be517b6ea18753`, then run from repository root:

```sh
python3 - <<'PYEX'
from pathlib import Path
s=Path('crates/spark-max-rs/src/lib.rs').read_text()
assert 'SparkMaxBindings::global()' in s
assert 'NativeRustFallback' in s and 'pub fn is_fallback' in s
print('MAX AND FALLBACK BRANCHES PRESENT')
PYEX
```

Expected that line, exit 0. This is a source-boundary lesson, not an inference test. A missing library is the negative branch; a true MAX model run requires pinned Modular, model weights and platform receipts, none exercised here. [Replay receipt](teaching-replay-2026-10-22.txt).


### Parent fresh-checkout replay

A separate coordination replay is retained in [the parent receipt](parent-replay-2026-10-22.txt). Its scope and observed exit are explicit; source review and final desk qualification remain separate. A recorded absence or expected failing baseline is not a passing implementation.
