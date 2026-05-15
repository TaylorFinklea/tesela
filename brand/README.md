# Tesela Brand Assets

Canonical source-of-truth for the Tesela logo mark and related brand assets.

## The mark

A mosaic-monogram **T** built from individual tile shapes (*tesserae*) — chosen because the product name *Tesela* is Spanish for the small tiles used in mosaics, and the UI itself is a grid of resizable panes (Prism v4 / tmux–zellij IA). The mark reinforces both concepts at a glance.

Design direction: **B1** from the 2026-05-15 Recraft batch (see `tesela-logo-mark.recraft.json` for generator metadata).

## Files

| File | Role |
|---|---|
| `tesela-logo-mark.svg` | **Vector master.** Single source of truth. Tight-cropped square (`viewBox="475 475 1342 1342"`), transparent background. Edit this when shape or framing changes are needed. |
| `tesela-logo-mark.png` | 1342×1342 raster preview rendered from the master. Hand-editable for one-off tweaks (e.g. the framing fix on 2026-05-15 was done here in Photoshop, then back-ported to the SVG's `viewBox`). |
| `tesela-logo-mark.recraft.json` | Recraft generation metadata (original prompt, style, settings). Reproducibility record only — not consumed by the build. Note: the original Recraft output had a white background and wider padding; both have since been corrected directly in the SVG master. |
| `og-image.svg` | OG/Twitter link preview composition (1200×630) — logo + wordmark + subtitle on cream canvas. Embeds the mark's path data directly (independent of the master's viewBox). |
| `exports/` | Pre-rendered PNGs of the mark at standard sizes (32, 180, 192, 512, 1024, 2048) and the OG image. The 2048 export is the high-res raster master — open it in Photoshop for one-off edits, then back-port any framing changes to the SVG. Regenerate with the commands in the *Regenerating exports* section. |
| `tesela-logo-mark-source.tiff` | ⚠️ **Stale.** Original 2292×2292 Recraft raster with the old wide-padding framing and a white background. Kept temporarily for reference; safe to delete. |

## Colors

| Role | Hex | Notes |
|---|---|---|
| Tiles | `#023047` | Deep navy. Reads on both light and dark UI backgrounds. |
| Accent | `#F13408` | Warm orange-red. Currently on the middle stem tile — repositionable in Photoshop or by re-coloring a different path in the SVG. |

## Where it ships

**Web (`web/static/`):**

| File | Source | Wired into |
|---|---|---|
| `favicon.svg` | copy of `tesela-logo-mark.svg` | `<link rel="icon" type="image/svg+xml">` |
| `favicon-32.png` | `exports/tesela-logo-32.png` | `<link rel="icon" sizes="32x32">` (legacy fallback) |
| `apple-touch-icon.png` | `exports/tesela-logo-180.png` | `<link rel="apple-touch-icon">` |
| `icon-192.png` | `exports/tesela-logo-192.png` | PWA manifest |
| `icon-512.png` | `exports/tesela-logo-512.png` | PWA manifest |
| `og-image.png` | `exports/tesela-og-image.png` | `og:image`, `twitter:image` |
| `manifest.webmanifest` | hand-authored | `<link rel="manifest">` |

**README:** `brand/exports/tesela-logo-512.png` is embedded at the top of the root `README.md`.

**iOS / macOS apps:** not yet — the iOS Xcode project (`app/Tesela-iOS/`) doesn't have an `Assets.xcassets` catalog set up, and no macOS app exists yet. Wire icons in when those targets get their asset catalogs.

When the SVG master changes, regenerate exports (see below) and re-copy into `web/static/`. No build script automates this yet — keep it manual until it hurts.

## Regenerating exports

From the `brand/` directory:

```sh
# Mark exports (2048 is the high-res raster master for Photoshop work)
for size in 32 180 192 512 1024 2048; do
  inkscape --export-type=png --export-filename="exports/tesela-logo-${size}.png" \
    -w ${size} -h ${size} tesela-logo-mark.svg
done

# Top-level preview
inkscape --export-type=png --export-filename="tesela-logo-mark.png" \
  -w 1342 -h 1342 tesela-logo-mark.svg

# OG image (text→paths so it renders without Inter Tight installed)
inkscape --export-type=png --export-filename="exports/tesela-og-image.png" \
  -w 1200 -h 630 --export-text-to-path og-image.svg

# Sync to web/static/
cp tesela-logo-mark.svg              ../web/static/favicon.svg
cp exports/tesela-logo-32.png        ../web/static/favicon-32.png
cp exports/tesela-logo-180.png       ../web/static/apple-touch-icon.png
cp exports/tesela-logo-192.png       ../web/static/icon-192.png
cp exports/tesela-logo-512.png       ../web/static/icon-512.png
cp exports/tesela-og-image.png       ../web/static/og-image.png
```

## Editing notes

- **Accent tile placement is not load-bearing.** The accent can be moved to any tile without re-running the generator. The optical sweet spots are (a) the rightmost crossbar tile and (b) the second-from-top tile in the stem. Avoid the very bottom of the stem — it makes the mark feel like it's leaking downward.
- **Don't add gradients, shadows, or 3D.** The mark is intentionally flat-vector so it survives at 16 px (favicon) and 1024 px (app icon) without losing its read.
- **Dark-mode adaptation (SVG favicon only).** The SVG embeds a `<style>` block that swaps the navy tile fill to off-white (`#E8EEF2`) under `@media (prefers-color-scheme: dark)`. The coral accent stays constant. PNG exports always render the light-mode default (navy on transparent) because Inkscape doesn't evaluate the dark-mode media query at render time — which is the desired behavior, since the rasters are used in contexts (apple-touch-icon, PWA, OG image) where the platform handles its own dark-mode treatment.

## Prompt of record

> Minimalist vector logo mark for an app called "Tesela." A single capital letter "T" constructed from 5–7 small irregular mosaic tiles (tesserae) with thin gaps between them, arranged on a square canvas. Flat 2D vector, geometric, no gradients, no shadows, no 3D. Two-tone: deep indigo tiles on transparent background, with one accent tile in warm coral to suggest a "focused" pane. Crisp edges, suitable for a 32×32 favicon and a 512×512 app icon. Clean, modern, slightly editorial — think Linear, Notion, Things 3.
