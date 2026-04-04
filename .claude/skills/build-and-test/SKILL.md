---
name: build-and-test
description: Run full Rust + Swift build and test suite with pass/fail per step
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

4. **Regenerate Xcode project**
   ```bash
   cd app/Tesela && xcodegen generate
   ```

5. **Swift build**
   ```bash
   xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build
   ```

## Output

Report a table:

| Step | Result |
|------|--------|
| cargo fmt | ✅/❌ |
| cargo clippy | ✅/❌ |
| cargo test | ✅/❌ (N tests) |
| xcodegen | ✅/❌ |
| xcodebuild | ✅/❌ |

If any step fails, show the error output and stop.
