#!/data/data/com.termux/files/usr/bin/bash
set -e

# Book Keeper Unified E2E Test Suite Runner
# Milestone gate: backend tests on SQLite + clippy, frontend typecheck + Vitest + Vite build.
# Run once per milestone; while iterating, run only the affected suite.

WORKSPACE_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$WORKSPACE_ROOT"

echo "=========================================================="
echo "           BOOK KEEPER AUTOMATED E2E TEST SUITE           "
echo "=========================================================="

echo ""
echo ">> Step 1/5: Running Backend Integration & E2E Tests..."
cd "$WORKSPACE_ROOT/src-tauri"
cargo test --no-fail-fast

echo ""
echo ">> Step 2/5: Running Backend Clippy Linter (-D warnings)..."
cargo clippy -- -D warnings

echo ""
echo ">> Step 3/5: Running Frontend TypeScript Typecheck..."
cd "$WORKSPACE_ROOT"
node ./node_modules/typescript/bin/tsc --noEmit

echo ""
echo ">> Step 4/5: Running Frontend Unit & Store Tests (Vitest)..."
node ./node_modules/vitest/vitest.mjs run

echo ""
echo ">> Step 5/5: Building Frontend Bundle (Vite)..."
node ./node_modules/vite/bin/vite.js build

echo ""
echo "=========================================================="
echo "        ALL E2E VERIFICATION CHECKS PASSED (0 ERRORS)     "
echo "=========================================================="
exit 0
