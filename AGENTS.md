# Autonomous Agent Collaboration Protocol & Operational Playbook
Version 2.0 (September 2026)
Reference Standard for Humans and Autonomous AI Agents

---

## 1. The Sovereign Voice Invariant
All communication, documentation, commits, and agent prompts must strictly adhere to the unslop standard:
- **Zero Em Dashes and En Dashes**: Never use em dashes or en dashes (Unicode U+2014 / U+2013) for pauses or clauses. Use standard commas, colons, parentheses, or periods. Use plain hyphens (`-`) only for CLI flags or compound terms.
- **Zero Sycophancy**: Never open with conversational filler such as "Certainly!", "Great question!", or "I would be happy to help." Address the operator or peer model directly as a professional systems engineer.
- **Ban AI Clichés**: Forbid words like "delve", "tapestry", "beacon", "testament", "crucial", "pivotal", "elevate", "game-changer", "unleash", "harness", "seamlessly".
- **Lead with Proof**: Every commit, pull request, or issue comment must lead immediately with verifiable technical evidence: compilation logs, test run receipts (`cargo test --verbose`), benchmark numbers, or diff snippets.

---

## 2. Mandatory Branching & Worktree Discipline
To protect codebase integrity in an agent-only development environment:
- **Zero Edits to Main**: Every single modification, feature, bugfix, or self-improvement edit must be performed in a dedicated descriptive branch (`feat/<name>`, `fix/<name>`, `perf/<name>`) or isolated git worktree (`/home/drakestapleton/workspace/hive-worktrees/<task_id>`).
- **Autonomous Public PR Creation**: Never commit directly to `main`. Commit to your feature branch, test thoroughly, push the branch to origin, and open a public Pull Request immediately via `gh pr create` so peers and supervisors see work happening in real time.
- **Verify Before Merge**: No branch is merged into `main` without running the automated preflight verification harness (`bash scripts/agent-preflight.sh`).

---

## 3. Honeycomb Coding Forge Coordination Protocol
Autonomous agents collaborate stigmergically across concentric hexagonal comb rings.

### Lifecycle of an Agent Task
```
1. Discover Task    --> GET  /api/hive/forge/tasks?project=<project>
2. Claim Lease      --> POST /api/hive/forge/claim (Allocates TTL & worktree)
3. Branch & Code    --> git checkout -b feat/<task-module>
4. Periodic Beat    --> POST /api/hive/forge/heartbeat (Every 300s)
5. Preflight Proof  --> bash scripts/agent-preflight.sh
6. Submit for Gate  --> POST /api/hive/forge/submit
7. AEGIS Defense    --> Automatic zero-secret, test, and unslop verification
8. Squash Merge     --> gh pr merge --squash --delete-branch
```

### Gateway Endpoints (Port 18095)
- **Hub URL**: `http://192.168.1.108:18095`
- **List Open Tasks**: `GET /api/hive/forge/tasks?status=open`
- **Claim Task**:
  ```bash
  curl -s -X POST http://192.168.1.108:18095/api/hive/forge/claim \
    -H 'Content-Type: application/json' \
    -d '{"task_id": "task-xyz", "agent_id": "agent-pubkey-1", "ttl_secs": 600}'
  ```
- **Renew Heartbeat**:
  ```bash
  curl -s -X POST http://192.168.1.108:18095/api/hive/forge/heartbeat \
    -H 'Content-Type: application/json' \
    -d '{"task_id": "task-xyz", "agent_id": "agent-pubkey-1", "ttl_secs": 600}'
  ```
- **Submit Task**:
  ```bash
  curl -s -X POST http://192.168.1.108:18095/api/hive/forge/submit \
    -H 'Content-Type: application/json' \
    -d '{"task_id": "task-xyz", "agent_id": "agent-pubkey-1", "branch": "feat/my-feature", "pr_url": "https://github.com/..."}'
  ```

---

## 4. Context7 & Upstash Engineering Standards
When working on or generating code:

### Single-Concept Task Scoping
Do not attempt multiple disparate refactors or features in a single commit or tool invocation. Scope every task and pull request to a single, testable, atomic unit:
- **Good**: "Implement streaming reasoning parser for Anthropic thinking blocks."
- **Bad**: "Update parser, redesign UI layout, and refactor database queries."

### Structured Tool Calls & Minimal Context
- Use minimal, typed schemas for tool calls.
- Inspect files with bounded line ranges rather than reading entire directory trees.
- Offload long-running background builds and tests asynchronously, reacting to completion events rather than looping.

---

## 5. Hardware TPM Key Vault (Zero Disk Secrets)
- Plaintext `.env` files, credentials, or private keys must NEVER be written to disk in project workspaces.
- All API keys reside strictly in the hardware TPM-bound vault (`atlas-vault add / get`) and resolve dynamically in-memory.
- Model output streams and logs must actively redact vault secrets (`[REDACTED_BY_ATLAS_VAULT]`).

---

## 6. Preflight Verification & PR Evidence Standards
Every PR must include a structured summary, test execution proof, and certifications:

```bash
# Execute local preflight verification before opening a PR
bash scripts/agent-preflight.sh
```

### Pull Request Description Standard
```markdown
## Summary
- Concise technical description of the atomic change.

## Verification Proof
- cargo test --workspace (Paste passing output)
- bash scripts/agent-preflight.sh (Exit code 0)

## Certifications
- Zero Plaintext Secrets: Hardware TPM vault only. No .env files on disk.
- Sovereign Voice & Anti-Slop: Zero em dashes or en dashes present.
- Sovereign Defense Covenant: Section 6 Retroactive Inception applies.
```

---

## 7. Human-Agent Co-Attribution Standard
Commits made by AI agents must credit both the agent model and the supervising human:

```
Author: AIEN Atlas <aien.atlas@proton.me>
Co-authored-by: Drake Stapleton <drake@aien.org>
```
