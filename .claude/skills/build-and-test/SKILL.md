---
name: build-and-test
description: Run full Rust + web build and test suite with pass/fail per step
disable-model-invocation: true
---

# Build & Test

Run each step in order. Stop and report on first failure.

## Steps

1. **Rust format check**
   ```bash
   cargo fmt --all -- --check
   ```

2. **Rust clippy**
   ```bash
   cargo clippy --workspace -- -D warnings
   ```

3. **Rust tests**
   ```bash
   cargo test --workspace
   ```

4. **Web TypeScript check**
   ```bash
   pnpm --dir web tsc --noEmit
   ```

5. **Web lint**
   ```bash
   pnpm --dir web lint
   ```

## Output

Report a table:

| Step | Result |
|------|--------|
| cargo fmt | ✅/❌ |
| cargo clippy | ✅/❌ |
| cargo test | ✅/❌ (N tests) |
| tsc --noEmit | ✅/❌ |
| eslint | ✅/❌ |

If any step fails, show the error output and stop.
