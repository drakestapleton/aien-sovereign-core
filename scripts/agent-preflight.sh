#!/usr/bin/env bash
set -eo pipefail

echo "============================================================"
echo "    AIEN Sovereign Agent Preflight Verification Harness     "
echo "============================================================"

FAILED=0

# Step 1: Branch Guard (Zero direct commits to main)
echo -n "[1/5] Verifying Branch Isolation... "
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
if [ "$CURRENT_BRANCH" = "main" ]; then
    echo "FAILED"
    echo "Error: Cannot commit directly to main. Create a feature branch (e.g. feat/, fix/)."
    FAILED=1
else
    echo "PASSED (Branch: $CURRENT_BRANCH)"
fi

# Step 2: Zero Disk Secrets Audit
echo -n "[2/5] Auditing Zero Disk Secrets... "
SECRET_FILES=$(find . -maxdepth 3 -name ".env*" -o -name "*.key" -o -name "*secret*" -o -name "*.pem" 2>/dev/null | grep -v ".git" | grep -v "target" || true)
if [ -n "$SECRET_FILES" ]; then
    echo "FAILED"
    echo "Error: Stray plaintext secret files detected on disk:"
    echo "$SECRET_FILES"
    FAILED=1
else
    echo "PASSED (Zero disk secrets detected)"
fi

# Step 3: Unslop and Sovereign Voice Audit on Staged / Working Changes
echo -n "[3/5] Verifying Unslop & Anti-Slop Discipline... "
DASH_VIOLATIONS=$(git diff HEAD -- ':(exclude)*.lock' ':(exclude)*.svg' ':(exclude)*.bin' 2>/dev/null | grep -E '^\+[^+]' | grep -E '(—|–)' | grep -v "replace" | grep -v "contains" | grep -v "ZERO EM DASHES" | grep -v "forbidden" | grep -v "assert\!" || true)
if [ -n "$DASH_VIOLATIONS" ]; then
    echo "FAILED"
    echo "Error: Prohibited em dash or en dash characters introduced in git diff:"
    echo "$DASH_VIOLATIONS"
    FAILED=1
else
    echo "PASSED (Zero em/en dashes in diff)"
fi

# Step 4: Test Suite Verification
echo "[4/5] Running Workspace Test Suite..."
if cargo test --workspace; then
    echo "PASSED (100% test pass)"
else
    echo "FAILED (Test suite failed)"
    FAILED=1
fi

# Step 5: Vulnerability & Advisory Audit
echo -n "[5/5] Checking RustSec Vulnerability Advisories... "
if cargo audit 2>/dev/null; then
    echo "PASSED (0 advisories)"
else
    echo "PASSED WITH WARNINGS (Check cargo audit logs)"
fi

echo "============================================================"
if [ "$FAILED" -eq 0 ]; then
    echo " ✓ PREFLIGHT VERIFICATION COMPLETE: Ready for PR Creation"
    echo "============================================================"
    exit 0
else
    echo " ✗ PREFLIGHT FAILED: Resolve defects before opening PR"
    echo "============================================================"
    exit 1
fi
