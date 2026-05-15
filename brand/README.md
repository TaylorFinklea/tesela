# Tesela Brand Assets

Canonical source-of-truth for the Tesela logo mark and related brand assets.

## The mark

A mosaic-monogram **T** built from individual tile shapes (*tesserae*) — chosen because the product name *Tesela* is Spanish for the small tiles used in mosaics, and the UI itself is a grid of resizable panes (Prism v4 / tmux–zellij IA). The mark reinforces both concepts at a glance.

Design direction: **B1** from the 2026-05-15 Recraft batch (see `tesela-logo-mark.recraft.json` for generator metadata). The original Recraft output had a white background and wide padding; both have since been corrected directly in the SVG masters. Framing: square `viewBox="375 375 1542 1542"` with ~10% padding around the mark.

## The variants

Tesela's icon ships at **two detail levels** (full T mark for big surfaces, simplified 3-tile mark for small surfaces) × **two color variants** (light navy for light surfaces, dark sky-blue for dark surfaces), plus a **card-backed PNG** for surfaces where the wallpaper is unknowable. The accent color `#F13408` stays constant across every combination — it's the continuity thread.

### Detail levels (responsive iconography)

| Level | When to use | Why |
|---|---|---|
| **Full mark** (7 tiles, T-shape) | ≥ 180 px — Apple touch icon, PWA icons, README header, OG image, landing page, marketing | At these sizes the mosaic gaps read and reward inspection. |
| **Simplified mark** (3 tiles — middle stem + bottom stem + coral accent) | ≤ 32 px — favicon, in-app top-bar mark | The full T's mosaic gaps disappear at favicon resolution; stripping to 3 bold shapes keeps the icon assertive instead of mushy. The simplified version reads as "two blue blocks with a coral interrupt" — distinct enough to recognize, simple enough to survive 16×16. |

### Color variants

| Variant | Tile color | When to use |
|---|---|---|
| **Light** (canonical) | `#023047` deep navy | Light surfaces: light-mode browser tabs, light README theme, OG image, landing page. Reads with ~14:1 contrast against white. |
| **Dark** | `#93C5FD` soft sky-blue (Tailwind blue-300) | Dark surfaces: dark-mode browser tabs, dark README theme, in-app branding (Tokyo Night and friends), future macOS/iOS dark icon variants. Reads with ~9:1 contrast against typical dark UI. |
| **Card-backed** | `#023047` navy on `#F8F7F4` cream rounded square | PNG-only contexts where the surface is unpredictable: iOS Home Screen via `apple-touch-icon`, PWA install icons. |

## Files

### Masters

**Full T mark** (`viewBox="375 375 1542 1542"`, all 7 tiles):

| File | Role |
|---|---|
| `tesela-logo-mark.svg` | **Light variant.** Plain SVG, navy tiles + coral accent, transparent background. Single source of truth for the canonical full mark. |
| `tesela-logo-mark-dark.svg` | **Dark variant.** Same path data, tile fill `#93C5FD`. Transparent background. |
| `tesela-icon-card.svg` | **Card-backed variant.** Light variant with a `#F8F7F4` cream rounded square baked behind the mark. Drives the home-screen PNGs (apple-touch, PWA). |

**Simplified mark** (`viewBox="710 910 880 880"`, 3 tiles — middle stem, bottom stem, coral accent):

| File | Role |
|---|---|
| `tesela-icon-mark.svg` | **Light variant.** Plain SVG, navy tiles + coral, transparent background. Used directly as `/tesela-icon-light.svg` in the web app. |
| `tesela-icon-mark-dark.svg` | **Dark variant.** Tile fill `#93C5FD`. Used as `/tesela-icon-dark.svg` in the web app, swapped via the `.dark` body class. |
| `tesela-favicon.svg` | **Favicon SVG.** Simplified paths + CSS `@media (prefers-color-scheme: dark)` to swap tile color at browser render time. This is what ships via `<link rel="icon" type="image/svg+xml">`. |

**Other:**

| File | Role |
|---|---|
| `og-image.svg` | OG / Twitter card (1200×630). Logo + wordmark + subtitle on cream canvas. Embeds the **full** mark's path data directly. |
| `tesela-logo-mark.recraft.json` | Recraft generation metadata for reproducibility. Not consumed by the build. |

### Exports

| Path | Source SVG | Contents |
|---|---|---|
| `exports/tesela-logo-{32,180,192,512,1024,2048}.png` | `tesela-logo-mark.svg` | Full-mark light PNGs (navy + transparent). 2048 is the high-res raster for Photoshop work. |
| `exports/dark/tesela-logo-dark-{32,192,512,1024,2048}.png` | `tesela-logo-mark-dark.svg` | Full-mark dark PNGs (light-blue + transparent). |
| `exports/card/tesela-icon-card-{180,192,512,1024}.png` | `tesela-icon-card.svg` | Full-mark card-backed PNGs (navy on cream rounded square). |
| `exports/icon/tesela-icon-{16,32,64,128,256,512}.png` | `tesela-icon-mark.svg` | Simplified-mark light PNGs (3 tiles, navy + coral). |
| `exports/icon/dark/tesela-icon-dark-{16,32,64,128,256,512}.png` | `tesela-icon-mark-dark.svg` | Simplified-mark dark PNGs. |
| `exports/tesela-og-image.png` | `og-image.svg` | OG/Twitter image (1200×630). |

## Colors

| Role | Hex | Notes |
|---|---|---|
| Light tile | `#023047` | Deep navy. The canonical mark color. |
| Dark tile | `#93C5FD` | Soft sky-blue (Tailwind blue-300). Dark-surface twin of the navy — same approximate hue, inverted luminance. |
| Accent | `#F13408` | Warm orange-red. Constant across both variants. The continuity thread. |
| Card backdrop | `#F8F7F4` | Cream. Same backdrop as the OG image — same cream appears across all "card" contexts. |

## Where it ships

### Web (`web/static/`)

| File | Source | Wired into |
|---|---|---|
| `favicon.svg` | `tesela-favicon.svg` (simplified, variant-swap) | `<link rel="icon" type="image/svg+xml">` — simplified 3-tile mark, adapts via CSS to light/dark browser chrome |
| `favicon-32.png` | `exports/icon/tesela-icon-32.png` (simplified light) | `<link rel="icon" sizes="32x32">` — small-size raster fallback |
| `tesela-icon-light.svg` | `tesela-icon-mark.svg` | In-app top-bar mark (`.v4-mark` in `routes/v4/+layout.svelte`) — served as a background-image, swapped to dark variant via `.dark` body class |
| `tesela-icon-dark.svg` | `tesela-icon-mark-dark.svg` | In-app top-bar mark — used when `:global(.dark)` body class is active |
| `apple-touch-icon.png` | `exports/card/tesela-icon-card-180.png` | `<link rel="apple-touch-icon">` — full mark, card-backed for any iOS wallpaper |
| `icon-192.png` | `exports/card/tesela-icon-card-192.png` | PWA manifest — full mark, card-backed |
| `icon-512.png` | `exports/card/tesela-icon-card-512.png` | PWA manifest |
| `og-image.png` | `exports/tesela-og-image.png` | `og:image`, `twitter:image` — full mark, cream backdrop baked in |
| `manifest.webmanifest` | hand-authored | `<link rel="manifest">` |

### README

The root `README.md` uses a `<picture>` element so GitHub serves the dark variant to readers on the dark theme:

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/exports/dark/tesela-logo-dark-512.png" />
  <img src="brand/exports/tesela-logo-512.png" alt="Tesela logo" width="128" height="128" />
</picture>
```

### iOS / macOS

Not yet wired. When the iOS asset catalog (`app/Tesela-iOS/Assets.xcassets`) is set up, ship both variants as the app icon's Light and Dark appearance entries. The card-backed PNGs are reserved for the Home Screen / Dock contexts where Apple's asset catalog can't fall back to.

## Regenerating exports

From the `brand/` directory:

```sh
# Light variant — navy tiles, transparent bg
for size in 32 180 192 512 1024 2048; do
  inkscape --export-type=png --export-filename="exports/tesela-logo-${size}.png" \
    -w ${size} -h ${size} tesela-logo-mark.svg
done
inkscape --export-type=png --export-filename="tesela-logo-mark.png" \
  -w 1542 -h 1542 tesela-logo-mark.svg

# Dark variant — light-blue tiles, transparent bg
for size in 32 192 512 1024 2048; do
  inkscape --export-type=png --export-filename="exports/dark/tesela-logo-dark-${size}.png" \
    -w ${size} -h ${size} tesela-logo-mark-dark.svg
done

# Card-backed variant — navy tiles on cream rounded square
for size in 180 192 512 1024; do
  inkscape --export-type=png --export-filename="exports/card/tesela-icon-card-${size}.png" \
    -w ${size} -h ${size} tesela-icon-card.svg
done

# Simplified mark — 3 tiles, used at favicon size and in-app top-bar
for size in 16 32 64 128 256 512; do
  inkscape --export-type=png --export-filename="exports/icon/tesela-icon-${size}.png" \
    -w ${size} -h ${size} tesela-icon-mark.svg
  inkscape --export-type=png --export-filename="exports/icon/dark/tesela-icon-dark-${size}.png" \
    -w ${size} -h ${size} tesela-icon-mark-dark.svg
done

# OG image (text→paths so it renders without Inter Tight installed)
inkscape --export-type=png --export-filename="exports/tesela-og-image.png" \
  -w 1200 -h 630 --export-text-to-path og-image.svg

# Sync to web/static/
cp tesela-favicon.svg                    ../web/static/favicon.svg
cp exports/icon/tesela-icon-32.png       ../web/static/favicon-32.png
cp tesela-icon-mark.svg                  ../web/static/tesela-icon-light.svg
cp tesela-icon-mark-dark.svg             ../web/static/tesela-icon-dark.svg
cp exports/card/tesela-icon-card-180.png ../web/static/apple-touch-icon.png
cp exports/card/tesela-icon-card-192.png ../web/static/icon-192.png
cp exports/card/tesela-icon-card-512.png ../web/static/icon-512.png
cp exports/tesela-og-image.png           ../web/static/og-image.png
```

## Editing notes

- **Accent tile placement is not load-bearing.** The accent can be moved to any tile without re-running the generator. The optical sweet spots are (a) the rightmost crossbar tile and (b) the second-from-top tile in the stem. Avoid the very bottom of the stem — it makes the mark feel like it's leaking downward.
- **Don't add gradients, shadows, or 3D.** The mark is intentionally flat-vector so it survives at 16 px (favicon) and 1024 px (app icon) without losing its read.
- **The accent color is the continuity thread.** When making a new variant (e.g. a high-contrast accessibility variant, a monochrome variant), keep `#F13408` constant. The blue tiles can shift; the orange anchors the brand.
- **Background-card adaptive favicon (deprecated 2026-05-15).** An earlier iteration adapted the favicon by adding a cream card only in dark mode. It worked but felt inconsistent with how the icon shipped to dark-context README and future in-app branding. Replaced with the variant-swap approach for unified behavior across all dark-context surfaces.

## Prompt of record

> Minimalist vector logo mark for an app called "Tesela." A single capital letter "T" constructed from 5–7 small irregular mosaic tiles (tesserae) with thin gaps between them, arranged on a square canvas. Flat 2D vector, geometric, no gradients, no shadows, no 3D. Two-tone: deep indigo tiles on transparent background, with one accent tile in warm coral to suggest a "focused" pane. Crisp edges, suitable for a 32×32 favicon and a 512×512 app icon. Clean, modern, slightly editorial — think Linear, Notion, Things 3.
