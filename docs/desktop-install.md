# Installing Tesela Desktop on another Mac

The desktop app distributes like TestFlight, but through GitHub Releases:
install once by hand, and every later release arrives through the app's
built-in updater (it checks
`https://github.com/TaylorFinklea/tesela/releases/latest/download/latest.json`
on launch).

## First install

1. Open the [latest release](https://github.com/TaylorFinklea/tesela/releases/latest)
   and download `Tesela.app.zip`.
2. Unzip it and drag `Tesela.app` into `/Applications`.
3. Launch it. The app is Developer ID-signed, notarized, and stapled, so
   Gatekeeper opens it without any right-click dance.

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
launch serving `/g`, and the updater manifest). `--dry-run` prints the plan
and validates versions/tooling without building; see `scripts/desktop-release.sh --help`.
