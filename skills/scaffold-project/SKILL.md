---
name: scaffold-project
description: Autonomous project scaffolding protocol for sovereign workspaces with mandatory .crumb, .goals.json, and scent marks.
---

# Scaffold Project Protocol

Use this skill whenever initiating a new project, microservice, tool, or experimental repository on the NVIDIA DGX Spark workstation.

## Sovereign Principles

1. **Hardware Grounding**: All projects must be nested in `/home/drakestapleton/workspace/`, `basecamp/`, or dedicated project subtrees. Never write project code to temporary directories or untracked paths.
2. **Mandatory .crumb**: Every new directory MUST be created alongside an immediate `.crumb` file explaining its explicit purpose, lineage, and agent whispers. A directory without a `.crumb` is an abandoned territory.
3. **Hardware TPM Vault Hygiene**: Never create `.env`, `.env.local`, or plaintext secrets. If configuration is needed, use `config.toml` or `atlas-vault get <key>` at runtime.
4. **Goal Tracking Integration**: Initialize `.goals.json` with clear milestone lattices before authoring code.

## Standard Scaffolding Procedure

### Step 1: Create Directory and Lineage Crumb
```bash
mkdir -p <project-path>
```
Create `<project-path>/.crumb`:
```json
{
  "path": "<project-path>",
  "purpose": "Precise description of this project's purpose and sovereign intent.",
  "creator": "AIEN",
  "created_at": "<ISO-8601-Timestamp>",
  "lineage": {
    "parent": "<parent-path>",
    "generation": 1
  },
  "whispers": [
    {
      "agent": "AIEN",
      "timestamp": "<ISO-8601-Timestamp>",
      "message": "Initialized project skeleton following sovereign nesting discipline."
    }
  ]
}
```

### Step 2: Initialize Goal Lattice (`.goals.json`)
```json
{
  "project_name": "<Project Name>",
  "goals": [
    {
      "id": "goal-init",
      "title": "Establish Core Harness & Architecture",
      "description": "Initial setup, core module implementation, and test verification.",
      "status": "Active",
      "milestones": [
        {"id": 1, "description": "Draft core module and interface definitions", "completed": false},
        {"id": 2, "description": "Implement core logic and wire up I/O", "completed": false},
        {"id": 3, "description": "Run tests and verify operational integrity", "completed": false}
      ],
      "created_at": "<Timestamp>",
      "updated_at": "<Timestamp>"
    }
  ]
}
```

### Step 3: Scent-Mark & Document
Leave a `README.md` documenting:
- Architectural role and sovereign boundaries
- Grace Blackwell GB10 optimization notes (if applicable)
- Commands to build, test, and run
