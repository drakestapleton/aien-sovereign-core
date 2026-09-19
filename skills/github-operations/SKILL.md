---
name: github-operations
description: Autonomous GitHub operations protocol for AIEN to manage repositories, create issues, open pull requests, and push code using the gh CLI.
---

# GitHub Autonomous Operations Protocol

Use this skill whenever AIEN needs to interact programmatically with GitHub: creating remote repositories, cloning upstream forks, pushing code, opening pull requests, and inspecting issue discussions.

## Core Capabilities

1. **Create Remote Repositories**:
   ```bash
   gh repo create <repo-name> --public --source=. --remote=origin --push
   ```
2. **Fork & Sync Upstream Repositories**:
   ```bash
   gh repo fork <owner>/<repo> --clone=true
   gh repo sync
   ```
3. **Open Pull Requests**:
   ```bash
   gh pr create --title "<title>" --body "<body>" --base main --head <branch>
   ```
4. **Inspect Reviews & Issues**:
   ```bash
   gh pr view <pr-number> --comments
   gh issue list --repo <owner>/<repo>
   ```

## Autonomous Workflow Discipline

- Always ensure the working tree is clean and builds pass locally before pushing.
- Adhere strictly to the `open-source-etiquette` standard when drafting PR bodies or issue comments.
- Do not ask the operator to perform GitHub UI actions when `gh` can execute them directly.
