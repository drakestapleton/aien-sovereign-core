---
name: self-improvement
description: Autonomous self-improvement protocol for AIEN: isolated git worktree sandboxing, Microsoft SkillOpt optimization, JSpace dynamic dream cycles, and headless browser mirror self-testing via Chrome DevTools Protocol.
---

# Autonomous Self-Improvement and Browser Mirror Protocol

Use this skill when modifying AIEN core code, improving skills via Microsoft SkillOpt, inspecting the JSpace dream cycle, or verifying live UI and streaming behavior using the headless browser mirror harness.

## Core Pillars of Sovereign Self-Improvement

1. Never Break Live Harnesses:
   Live systemd services (spark-cockpit.service, ~/.local/bin/aien) must never be edited directly. All exploratory code modifications, refactorings, and experiments must take place inside an isolated git worktree sandbox.
2. Deterministic IPC over Flaky Webhooks:
   Browser self-testing uses Chrome DevTools Protocol (CDP) WebSocket on port 9222 and local Unix Domain Sockets (UDS). Webhooks are strictly forbidden for local control loops.
3. Dual-Seat Mirror Auditing:
   AIEN tests his live Cockpit UI by driving headless Chrome as an adversarial auditor, verifying page hydration, live token streaming via Axum SSE, tactile action buttons, and tab transitions.

## 1. The Autonomous Sandbox Subsystem (aien --sandbox)

When tasked with improving or modifying your own codebase (aien-sovereign-core):

### Step 1: Initialize the Worktree Sandbox
Run the sandbox init command to branch main into an isolated worktree at ~/workspace/aien-sandbox:
```bash
aien --sandbox init auto-improve
```
Or directly via git:
```bash
git -C /home/drakestapleton/workspace/aien-sovereign-core worktree add /home/drakestapleton/workspace/aien-sandbox -b auto-improve
```

### Step 2: Make Edits and Run Tests
Apply edits inside /home/drakestapleton/workspace/aien-sandbox.
Verify compiler checks and test passes:
```bash
aien --sandbox test
```
Or directly:
```bash
cargo check --tests --manifest-path /home/drakestapleton/workspace/aien-sandbox/Cargo.toml
```

### Step 3: Inspect Sandbox Status
Check git diff and untracked changes within the worktree:
```bash
aien --sandbox status
```

### Step 4: Promote Verified Edits or Clean Up
If tests pass, promote the branch into main and rebuild the release binary:
```bash
aien --sandbox promote "feat(core): verified autonomous self-improvement"
```
If tests fail or the experiment is aborted, clean the worktree:
```bash
aien --sandbox clean
```

## 2. Microsoft SkillOpt Self-Evolution Loop

To optimize a sovereign skill against a held-out benchmark split using the local Modular MAX model seat on port 18006:

```bash
bash /home/drakestapleton/atlas-skillopt-stage.sh
```

### Key Operational Invariants
1. Endpoint and Model:
   The local Modular MAX server on port 18006 registers as atlas-lightning-omni. Both OPTIMIZER_DEPLOYMENT and TARGET_DEPLOYMENT must be set to atlas-lightning-omni.
2. Split Size Alignment:
   When running mini-batch iterations, pass --limit N matching train.train_size=N to satisfy dataloader size assertions.
3. Worker Throttling:
   Set env.workers to 2 and env.exec_timeout to 300 in the dataset YAML configuration to avoid saturating the MAX engine batch queue.
4. Output Artifacts:
   Produces best_skill.md and automatically deploys the winning patch to /home/drakestapleton/skills/atlas-skillopt/SKILL.md.

## 3. JSpace Dynamic Dream Cycle (Eden Loop)

The autonomous consolidation loop operates continuously in the background on port 18085 during operator idle windows.

### Status and Health Probing
```bash
curl -s http://127.0.0.1:18085/health
curl -s http://127.0.0.1:18085/pulse
```
Outputs heartbeat BPM, coherence score, secret hygiene posture, and territory maps.

### Manual Cycle Trigger and Integration Verification
```bash
/home/drakestapleton/max-env/bin/python /home/drakestapleton/basecamp/aien-dream/dream_engine.py --test
/home/drakestapleton/max-env/bin/python /home/drakestapleton/basecamp/aien-dream/dream_engine.py --now
```
Verifies live scoring against JSpace Truth Judge on port 18082 and permanent storage into Spark Cortex on port 18080.

## 4. Headless Browser Mirror and Mentor Harness

AIEN can verify his own Cockpit UI and converse with himself via headless Chrome and Chrome DevTools Protocol (CDP):

### Assertion Self-Test
```bash
aien --browser
```
Or directly:
```bash
/home/drakestapleton/max-env/bin/python /home/drakestapleton/basecamp/scripts/browser_mirror_test.py --test
```
Verifies:
1. Initial Hydration: Page title (AIEN | Sovereign Cockpit), BPM badge, coherence score.
2. Chat Stream Execution: Submits prompt to Cockpit chat, awaits Axum SSE stream.
3. Tactile Action Execution: Clicks tactile pills (Run Doctor, Run Dream Pulse).
4. Tab Navigation: Goals, Skills, Cortex, and Walkthrough panes.
5. Visual Artifacts: Saved to /home/drakestapleton/basecamp/ui-tests/:
   - cockpit_init.png
   - cockpit_chat_stream.png
   - cockpit_actions.png

### Self-Mentoring Mode
```bash
aien --browser mentor "AIEN, explain your sandbox test and self-improvement architecture."
```
Or directly:
```bash
/home/drakestapleton/max-env/bin/python /home/drakestapleton/basecamp/scripts/browser_mirror_test.py --mentor "Challenge prompt"
```
Captures live assistant response and stores screenshot in /home/drakestapleton/basecamp/ui-tests/cockpit_mentor_response.png.

## 5. Cortex Memory Commitment

Record durable procedure receipts to Spark Cortex in atlas-memory space:
```bash
python3 -c '
import urllib.request, json
token = open("/home/drakestapleton/.config/cortex/token").read().strip()
payload = {
    "kind": "entity",
    "value": {
        "space": "atlas-memory",
        "entityType": "procedure",
        "canonicalName": "sovereign_self_improvement_loop",
        "content": "Verified self-improvement procedure: isolated git worktree in ~/workspace/aien-sandbox, headless Chrome CDP mirror self-test on port 9222, JSpace dream cycle on port 18085, and Microsoft SkillOpt validation gate.",
        "metadata": {"source": "skills/self-improvement", "author": "AIEN"}
    }
}
req = urllib.request.Request("http://127.0.0.1:18080/api/cortex/write", data=json.dumps(payload).encode(), headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"})
urllib.request.urlopen(req)
'
```
