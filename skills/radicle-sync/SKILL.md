---
name: radicle-sync
description: Peer-to-peer sovereign code synchronization using Radicle without external centralized forge dependencies.
---

# Radicle Sovereign Synchronization Protocol

Use this skill when synchronizing codebases, publishing cryptographic patches, or collaborating peer-to-peer across sovereign nodes.

## Sovereign Principles

1. **No External Centralized Forges**: Code remains local or synchronizes strictly via Radicle P2P seed nodes.
2. **Cryptographic Identity**: Node keys are managed under sovereign identity without plaintext passphrase storage.

## Standard Operations

### 1. Check Node Status & Identity
```bash
rad node status
rad self
```

### 2. Initialize a Repository in Radicle
```bash
cd <project-path>
rad init --name <project-name> --description "<description>" --default-branch main
```

### 3. Sync with Peer Swarm
```bash
rad sync
```
