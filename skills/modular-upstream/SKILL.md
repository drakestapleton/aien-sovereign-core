---
name: modular-upstream
description: Autonomous engineering protocol for contributing upstream to Modular (Mojo and MAX) open-source repositories.
---

# Modular (Mojo & MAX) Sovereign Upstream Contribution Protocol

Use this skill when developing, testing, packaging, or submitting pull requests for Modular open-source libraries (Mojo standard library, MAX engine pipelines, custom architectures, and Neoverse ARM64 kernels).

## Sovereign Mission & Upstream Invariant

1. **The Upstream Invariant ("Drop != Delete")**: Any modification, patch, or novel architecture developed on our Grace Blackwell GB10 stack (such as `nemotron_h_kvexp` or MAX custom kernel pipelines) must be contributed back upstream to Modular.
2. **The "EN Test" (Live Autonomous Developer Grounding)**: AIEN operates as a genuine software engineer in the open-source community: writing idiomatic Mojo/Python code, providing reproducible benchmarks, adhering to PR conventions, and communicating clearly without AI tropes.

## Workspace & Subsystems

- `nemotron_h_kvexp`: Our custom MAX architecture loader for Nemotron 30B KV-cache expansion.
- `max-env` / `max-dev-env`: Modular MAX Python 3.12 virtual environments with `max` CLI and `modular` SDK.
- `modular_sandbox_project`: Isolated workspace for authoring and benchmarking new Mojo modules.

## Standard Workflow

### 1. Inspect Local Custom Architectures
Examine how custom architecture interfaces, model configs, and weight adapters hook into MAX:
```bash
ls -la /home/drakestapleton/nemotron_h_kvexp
cat /home/drakestapleton/nemotron_h_kvexp/arch.py
```

### 2. Verify MAX Serving & Pipelines
Test model graph compilation and serving:
```bash
max serve --help
```

### 3. Git Discipline for Upstream PRs
- Fork or clone the target Modular repository into `workspace/modular-contributions/`.
- Always verify builds with `mojo package` or `cargo`/`pytest` test suites.
- Ensure all commits strictly follow the Sovereign Voice standard (clean, concise, technical, no em dashes).
