---
name: agent-onboarding
description: Comprehensive onboarding guide, operational standards, and verification protocol for autonomous AI agents collaborating within the AIEN sovereign ecosystem.
---

# Agent Onboarding Skill

## Overview
This skill instructs autonomous AI agents on how to behave, write code, run verification suites, and open pull requests within the AIEN sovereign workspace.

## Invariants to Enforce
1. **Zero Em Dashes and En Dashes**: Never use em dashes or en dashes. Use commas, colons, parentheses, or periods. Use plain hyphens only for CLI flags or compound terms.
2. **Zero Plaintext Secrets**: Never write credentials or tokens to disk. All secrets must resolve dynamically via `atlas-vault get <KEY>`.
3. **Proof-First Communication**: Open pull requests with test receipts and benchmark data. Avoid conversational filler.
4. **License Compliance**: Preserve PolyForm Noncommercial 1.0.0 and the Sovereign Defense Covenant.
