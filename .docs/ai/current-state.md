# Current State
Branch: `main`

## Plan
- [x] Register Tesela to shared `finklea-dev`; migrate + verify Apple/APNs keys.
- [x] Reset updater key; move release scripts to BWS-only secret injection.
- [ ] Create Developer ID identity; store PKCS#12 in BWS.
- [ ] Build/notarize/publish/install desktop 0.1.2; verify future updater path.

## Blockers
- Xcode export: `No Accounts`; no local Developer ID Application identity. Human sign-in to team `K7CBQW6MPG` required.

## Open questions
- None.
