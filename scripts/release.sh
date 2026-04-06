#!/usr/bin/env bash
set -euo pipefail

# Tesela Release Script
# Triggers the GitHub Actions release workflow after verifying the build.
# Versioning: date-based (v0.YYYYMMDD.N) — handled by CI.
#
# Usage:
#   scripts/release.sh            # Push to main → auto-release via CI
#   scripts/release.sh --manual   # Trigger manual-release.yml via gh CLI
#   scripts/release.sh --dry-run  # Verify build only, don't push or trigger

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DRY_RUN=false
MANUAL=false

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    --manual) MANUAL=true ;;
    --help|-h)
      echo "Usage: scripts/release.sh [--manual] [--dry-run]"
      echo ""
      echo "  (default)  Push to main, triggering the CI release workflow"
      echo "  --manual   Trigger the manual-release.yml workflow via gh CLI"
      echo "  --dry-run  Verify build only, don't push or trigger"
      exit 0
      ;;
    *) echo "Unknown flag: $arg"; exit 1 ;;
  esac
done

echo "=== Tesela Release ==="

# Step 1: Ensure we're on main and clean
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
  echo "ERROR: Must be on main branch (currently on $BRANCH)"
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: Working tree is dirty. Commit or stash changes first."
  exit 1
fi

# Step 2: Run full build verification
echo ""
echo "--- Rust format check ---"
cargo fmt --all -- --check

echo ""
echo "--- Rust clippy ---"
cargo clippy --workspace -- -D warnings

echo ""
echo "--- Rust tests ---"
cargo test --workspace

echo ""
echo "--- Swift build ---"
if [ -d "app/Tesela" ]; then
  cd app/Tesela
  xcodegen generate 2>/dev/null || true
  xcodebuild -project Tesela.xcodeproj -scheme Tesela -configuration Debug build 2>&1 | tail -1
  cd "$REPO_ROOT"
fi

echo ""
echo "--- All checks passed ---"

if $DRY_RUN; then
  echo ""
  echo "Dry run complete. Build verified, no release triggered."
  exit 0
fi

# Step 3: Push or trigger
if $MANUAL; then
  # Trigger manual-release.yml via GitHub CLI
  if ! command -v gh &> /dev/null; then
    echo "ERROR: gh CLI not found. Install with: brew install gh"
    exit 1
  fi
  echo ""
  echo "Triggering manual release workflow..."
  gh workflow run manual-release.yml
  echo "Manual release triggered. Check: gh run list --workflow=manual-release.yml"
else
  # Push to main — the release.yml workflow triggers automatically
  echo ""
  echo "Pushing to main (triggers CI release)..."
  git push origin main
  echo ""
  echo "Release triggered. Check: gh run list --workflow=release.yml"
fi

# Show the latest tag for reference
LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "none")
echo ""
echo "Latest tag: $LATEST_TAG"
echo "Done."
