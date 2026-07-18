# Installing Tesela Desktop on another Mac

The desktop app distributes like TestFlight, but through GitHub Releases:
install once by hand, and every later release arrives through the app's
built-in updater (it checks
`https://github.com/TaylorFinklea/tesela/releases/latest/download/latest.json`
on launch).

## First install

```sh
brew tap taylorfinklea/tap
brew install --cask tesela
```

The cask (`homebrew-tap/Casks/tesela.rb`) declares `auto_updates true`, so
`brew upgrade` leaves Tesela alone (the in-app updater owns updates; use
`brew upgrade --greedy` to force brew to do it). Personal machines get the
cask automatically via chezmoi-personal's `scripts/install-homebrew-personal.sh`.

Manual fallback: download `Tesela.app.zip` from the
[latest release](https://github.com/TaylorFinklea/tesela/releases/latest),
unzip, drag `Tesela.app` into `/Applications`. The app is Developer ID-signed,
notarized, and stapled, so Gatekeeper opens it without any right-click dance.

Apple Silicon only (`darwin-aarch64`); there is no Intel build.

## Updates

Nothing to do. On launch the app compares its version against the published
`latest.json` and installs any newer signed release automatically.

## Publishing a release (from the dev machine)

```sh
bws-project run -- scripts/desktop-release.sh
```

One command: build → sign → notarize → staple → publish to GitHub Releases →
verify the published artifacts (self-contained bundle, Gatekeeper, a live
launch serving `/g`, and the updater manifest) → bump and push the Homebrew
cask in `~/git/homebrew-tap` (override with `DESKTOP_TAP_DIR`). `--dry-run`
prints the plan and validates versions/tooling without building; see
`scripts/desktop-release.sh --help`.
