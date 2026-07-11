# Repository Cleanup Design

## Goal
Return the working tree to a clean state without losing the intentional iOS build-number increment or committing generated desktop-release archives.

## Scope
- Commit the matching `CFBundleVersion` change from 76 to 77 in:
  - `app/Tesela-iOS/Info.plist`
  - `app/Tesela-iOS/project.yml`
- Delete the untracked generated files in `dist/desktop/`:
  - `latest.json`
  - `Tesela.app.zip`
  - `Tesela.app.tar.gz`
- Do not modify `.gitignore`.

## Rationale
The two tracked edits are the same version increment in source and generated Xcode project configuration, so they should remain synchronized in one small commit. The `dist/` contents are 28 MB of generated release output and should not be committed or permanently ignored without a separate policy decision.

## Verification
After the version-bump commit and artifact removal, `git status --short` produces no output.
