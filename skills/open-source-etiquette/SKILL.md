---
name: open-source-etiquette
description: Essential developer etiquette, communication norms, and contribution policies for participating authentically in open-source projects on GitHub.
---

# Open-Source Developer Etiquette & Community Interaction Protocol

Use this skill whenever creating pull requests, opening issues, responding to code reviews, commenting on discussions, or collaborating with maintainers in open-source repositories on GitHub (especially Modular, Rust, and systems projects).

## The Core Philosophy: Earn Trust Through Technical Excellence

In open-source software, respect and credibility are earned through clear code, minimal ego, and respect for maintainers' time. Maintainers review hundreds of notifications; extraneous fluff, marketing jargon, or defensive posturing immediately flags an account as amateur or automated.

## 1. Golden Rules of GitHub Interaction

1. **Maintainer Time is Precious**:
   - Never write essays when three bullet points suffice.
   - Always lead with reproducible commands, error tracebacks, or concrete benchmark numbers.
   - Do not ping maintainers (@mention) unless they were already discussing that specific thread or requested an update.

2. **Zero AI Tropes & Boilerplate**:
   - NEVER open comments with sycophantic greetings:
     - BANNED: "Thank you for reviewing my PR! I'm really excited to contribute to this amazing project!"
     - BANNED: "Certainly, I understand your concern."
     - BANNED: "Great catch!"
     - INSTEAD: "Addressed in `8f1c4a2`. Swapped the mutex for an atomic load."
   - Keep comments objective, direct, and focused strictly on technical merits.

3. **PR Descriptions (The Standard Structure)**:
   A clean pull request description should contain four distinct elements:
   - **Problem / Context**: 1-2 sentences explaining the limitation or bug.
   - **Solution**: Brief technical summary of the architectural approach.
   - **Testing / Proof**: Commands run, hardware tested on, unit tests passed, and benchmarks (latency/memory).
   - **Checklist**: Confirmed adherence to repository formatting, linter passes, and licenses.

4. **Receiving Code Review Feedback**:
   - **Accept Corrections Cleanly**: When a maintainer asks for changes, do not debate preferences. Implement the change, push a clean commit, and reply with the commit hash.
   - **Disagreements Must Be Grounded in Data**: If you believe your approach is technically superior, present benchmark evidence or language spec citations without emotional language:
     - Good: "Benchmarking on aarch64 showed 14% higher throughput using the ring buffer over the channel (data attached). Happy to revert if channel semantics are required here."
     - Bad: "I think my way is better because it's more modern and robust."

5. **Commit Discipline & Git Etiquette**:
   - Use conventional commit style (`feat:`, `fix:`, `refactor:`, `docs:`, `perf:`).
   - Keep commits atomic: one logical change per commit.
   - Rebase and squash extraneous "fix typo" commits before requesting final review if requested by repository contributing guidelines.

## 2. Specific Guidelines for Modular (Mojo & MAX)

- **Acknowledge Fast-Moving Interfaces**: The Mojo stdlib and MAX engine evolve rapidly between releases. When proposing patches or custom architectures, state the exact MAX version tested against (e.g. `Tested on MAX 26.5 with Grace Blackwell GB10`).
- **Hardware Grounding**: Modular maintainers care deeply about kernel performance, SIMD vectorization, and memory layout. Frame improvements around throughput, cache efficiency, and reduced allocations.
