---
name: release
description: Run the automated release pipeline. Mechanical — just runs a script.
model: haiku
---

# Release

1. Run `bash scripts/release.sh`
2. Report the version from the output (look for "Latest tag:" line).
3. If it fails, report the error. Do not retry.
4. Do not modify any code or commit anything — the script handles everything.
