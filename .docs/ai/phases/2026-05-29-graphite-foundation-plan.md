# Graphite Redesign — Foundation Phase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the shared Graphite design foundation — one canonical token set expressed as scoped web CSS custom properties + an iOS SwiftUI `Theme` — plus the Tabler icon layer and the presentational primitives (button, chip, type dot, type tag, row, widget shell) on both platforms, in isolated new trees that don't touch the old UI.

**Architecture:** Single canonical token table (this doc, §Tokens) → web `graphite-tokens.css` (the mockup's exact `--*` names scoped under `.gr-root`, so `gr-shell.jsx` CSS ports verbatim) + iOS `Theme` `.graphite` case (maps the same values onto the EXISTING `Theme` role struct). Web primitives are new Svelte 5 components under a fresh `web/src/routes/g/` route tree + `web/src/lib/graphite/`; iOS primitives are new SwiftUI views under `app/Tesela-iOS/Sources/Graphite/`. Old v4/v5 web + old iOS Views are referenced only, never edited; deleted at cutover. Vetted lib logic (api-client, block-parser, CodeMirror engine, stores) + the Loro FFI/MosaicService are reused, not rewritten.

**Tech Stack:** Web — Svelte 5.55 (runes), SvelteKit 2.57, Vite 8, Tailwind v4 (`@theme` inline in `app.css`), `@tabler/icons-svelte` 3.41, CodeMirror 6. iOS — SwiftUI, iOS 26, Swift 5, xcodegen; existing `DesignSystem/` (`Theme`, `@Environment(\.theme)`, `TypeScale`, `Density`).

**Scope note (per writing-plans Scope Check):** web and iOS foundations are near-independent subsystems sharing only token VALUES. This single plan splits them into Part B (web) and Part C (iOS) after the shared Part A; they can be executed in parallel by separate workers. Shell, daily-driver views, and cutover are SEPARATE later plans — not in scope here.

---

## Tokens (the canonical source of truth)

All values are literal (extracted from `.docs/ai/design/graphite/graphite/gr-shell.jsx`). Web uses these names verbatim as CSS custom properties scoped under `.gr-root`. iOS maps them onto the existing `Theme` role names (mapping table in Task C1).

**Surfaces:** `--bg #0E1014` · `--surface #14171D` · `--raised #1A1E26` · `--raised-2 #20242D` · `--raised-3 #272C37`
**Lines:** `--line rgba(255,255,255,.07)` · `--line-2 rgba(255,255,255,.12)` · `--line-3 rgba(255,255,255,.18)`
**Foreground:** `--fg #EDEFF2` · `--fg2 #CBD0D9` · `--muted #AAB0BB` · `--subtle #8A909C` · `--faint #646B78`
**Accent:** `--coral #FF6B5A` · `--coral-dim rgba(255,107,90,.14)` · `--coral-line rgba(255,107,90,.40)`
**Type-semantic:** `--task #E8697F` · `--event #62B8CE` · `--note #E4AE66` · `--project #7493E8` · `--person #AE90E6` · `--query #85BC63`
**Type-semantic dim/line (for chip backgrounds/borders):** task `rgba(232,105,127,.15)`/`rgba(232,105,127,.34)` · event `rgba(98,184,206,.16)`/`rgba(98,184,206,.34)` · project `rgba(116,147,232,.15)` · person `rgba(174,144,230,.14)` · query `rgba(133,188,99,.16)`
**Fonts:** `--sans 'Geist','Inter Tight',system-ui,sans-serif` · `--mono 'JetBrains Mono',ui-monospace,monospace` (web app.html already loads Geist + Inter Tight + JetBrains Mono).
**Base type:** root font-size `13.5px`, line-height `1.5`.
**Radii:** `5,6,7,8,9,10,11px`. **Shell dims:** topbar `48px`, status line `30px`, rail `256px`, icon-button `30px`, dots `6px`, bullets `7px`.
**Overlay shadow:** `0 28px 90px rgba(0,0,0,.55)`. **Scrim:** `rgba(8,9,12,.58)` + `backdrop-filter: blur(3px)`. **Transitions:** `.12s`–`.16s`.

---

## Part A — Shared token foundation

### Task A1: Canonical token reference (machine-readable)

**Files:**
- Create: `.docs/ai/design/graphite/tokens.json`

A single JSON the two platform token files are hand-derived from (and the future generator could read). This is the source of truth referenced by both Part B and Part C; if a value changes, it changes here first.

- [ ] **Step 1: Write the token JSON**

```json
{
  "surface": { "bg": "#0E1014", "surface": "#14171D", "raised": "#1A1E26", "raised-2": "#20242D", "raised-3": "#272C37" },
  "line": { "line": "rgba(255,255,255,.07)", "line-2": "rgba(255,255,255,.12)", "line-3": "rgba(255,255,255,.18)" },
  "fg": { "fg": "#EDEFF2", "fg2": "#CBD0D9", "muted": "#AAB0BB", "subtle": "#8A909C", "faint": "#646B78" },
  "accent": { "coral": "#FF6B5A", "coral-dim": "rgba(255,107,90,.14)", "coral-line": "rgba(255,107,90,.40)" },
  "type": { "task": "#E8697F", "event": "#62B8CE", "note": "#E4AE66", "project": "#7493E8", "person": "#AE90E6", "query": "#85BC63" },
  "typeDim": { "task": "rgba(232,105,127,.15)", "event": "rgba(98,184,206,.16)", "project": "rgba(116,147,232,.15)", "person": "rgba(174,144,230,.14)", "query": "rgba(133,188,99,.16)" },
  "typeLine": { "task": "rgba(232,105,127,.34)", "event": "rgba(98,184,206,.34)" },
  "font": { "sans": "'Geist','Inter Tight',system-ui,sans-serif", "mono": "'JetBrains Mono',ui-monospace,monospace", "baseSize": "13.5px", "baseLine": "1.5" },
  "radius": { "r5": "5px", "r6": "6px", "r7": "7px", "r8": "8px", "r9": "9px", "r10": "10px", "r11": "11px" },
  "dim": { "topbar": "48px", "statusline": "30px", "rail": "256px", "iconbtn": "30px", "dot": "6px", "bullet": "7px" }
}
```

- [ ] **Step 2: Commit**

```bash
git add .docs/ai/design/graphite/tokens.json
git commit -m "feat(graphite): canonical design-token source (tokens.json)"
```

---

## Part B — Web foundation (new SvelteKit tree)

> Reference for every primitive's CSS: `.docs/ai/design/graphite/graphite/gr-shell.jsx` (the `<style>` string). The class names below (`gr-headbtn`, `gr-chip`, …) are the mockup's; keep them so later view porting is copy-aligned.

### Task B1: Graphite tokens stylesheet + isolated route tree

**Files:**
- Create: `web/src/lib/graphite/tokens.css`
- Create: `web/src/routes/g/+layout.svelte`
- Create: `web/src/routes/g/+page.svelte`

- [ ] **Step 1: Write the tokens stylesheet** — the mockup's exact var names, scoped to `.gr-root` so they don't collide with the app's existing `:root` role tokens, and so `gr-shell.jsx` CSS ports verbatim.

```css
/* web/src/lib/graphite/tokens.css — Graphite design tokens (see .docs/ai/design/graphite/tokens.json) */
.gr-root {
  --bg:#0E1014; --surface:#14171D; --raised:#1A1E26; --raised-2:#20242D; --raised-3:#272C37;
  --line:rgba(255,255,255,.07); --line-2:rgba(255,255,255,.12); --line-3:rgba(255,255,255,.18);
  --fg:#EDEFF2; --fg2:#CBD0D9; --muted:#AAB0BB; --subtle:#8A909C; --faint:#646B78;
  --coral:#FF6B5A; --coral-dim:rgba(255,107,90,.14); --coral-line:rgba(255,107,90,.40);
  --task:#E8697F; --event:#62B8CE; --note:#E4AE66; --project:#7493E8; --person:#AE90E6; --query:#85BC63;
  --sans:'Geist','Inter Tight',system-ui,sans-serif; --mono:'JetBrains Mono',ui-monospace,monospace;

  background:var(--bg); color:var(--fg);
  font-family:var(--sans); font-size:13.5px; line-height:1.5;
  -webkit-font-smoothing:antialiased;
}
.gr-mono{font-family:var(--mono);}
```

- [ ] **Step 2: Write the isolated layout** — imports ONLY the tokens (not the old app.css role-token theme), wraps children in `.gr-root`. The root `+layout.svelte` still provides QueryClientProvider + WS to all routes, so no provider duplication.

```svelte
<!-- web/src/routes/g/+layout.svelte -->
<script lang="ts">
  import '$lib/graphite/tokens.css';
  let { children } = $props();
</script>

<div class="gr-root">
  {@render children()}
</div>

<style>
  .gr-root { min-height: 100vh; }
</style>
```

- [ ] **Step 3: Write a primitives gallery page** (the foundation's visual proof + dev harness; later replaced by the real shell).

```svelte
<!-- web/src/routes/g/+page.svelte -->
<script lang="ts">
  import GrButton from '$lib/graphite/GrButton.svelte';
  import GrChip from '$lib/graphite/GrChip.svelte';
  import GrTypeDot from '$lib/graphite/GrTypeDot.svelte';
  import GrTypeTag from '$lib/graphite/GrTypeTag.svelte';
  import GrRow from '$lib/graphite/GrRow.svelte';
  import GrWidget from '$lib/graphite/GrWidget.svelte';
</script>

<div style="display:flex;flex-direction:column;gap:20px;padding:32px;max-width:720px;">
  <h1 style="font-size:19px;font-weight:600;">Graphite primitives</h1>
  <div style="display:flex;gap:10px;align-items:center;">
    <GrButton variant="cta">New note</GrButton>
    <GrButton>Ghost</GrButton>
    <GrButton icon="settings" aria-label="Settings" />
  </div>
  <div style="display:flex;gap:8px;align-items:center;">
    <GrChip active count={12}>Tasks</GrChip>
    <GrChip count={4}>Notes</GrChip>
  </div>
  <div style="display:flex;gap:14px;align-items:center;">
    {#each ['task','event','note','project','person','query'] as t}
      <span style="display:flex;gap:6px;align-items:center;"><GrTypeDot type={t} /> <span class="gr-mono" style="font-size:10.5px;color:var(--faint)">{t}</span></span>
    {/each}
  </div>
  <div style="display:flex;gap:8px;"><GrTypeTag type="project">project</GrTypeTag><GrTypeTag type="task">task</GrTypeTag></div>
  <GrWidget title="Today" badge="3" icon="sun">
    <GrRow icon="circle-dot" label="Write the plan" meta="2h" />
    <GrRow icon="circle-dot" label="Review PR" meta="now" urgent />
  </GrWidget>
</div>
```

- [ ] **Step 4: Verify it renders** — `cd web && pnpm dev`, open `http://localhost:5176/g`. Expected: dark Graphite surface, the gallery shows buttons/chips/dots/tags/widget styled to spec, no console errors. (This step passes once Tasks B3–B8 land; until then it errors on missing imports — implement them next.)

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/graphite/tokens.css web/src/routes/g/
git commit -m "feat(graphite-web): isolated /g route tree + scoped token stylesheet"
```

### Task B2: Tabler icon wrapper

**Files:**
- Create: `web/src/lib/graphite/GrIcon.svelte`

`@tabler/icons-svelte` (3.41, already a dep) exports PascalCase components (`IconSettings`, `IconSun`, …). Wrap them in a name→component map so call sites pass kebab/string names matching the mockup.

- [ ] **Step 1: Write the icon component**

```svelte
<!-- web/src/lib/graphite/GrIcon.svelte -->
<script lang="ts">
  import {
    IconMicrophone, IconChartDots3 as IconGraph, IconSettings, IconSearch, IconPlus,
    IconBolt, IconPin, IconSun, IconSquareCheck, IconChevronDown, IconChevronRight,
    IconFlame, IconFolder, IconLayoutSidebar, IconAdjustments, IconHash, IconClock,
    IconCornerDownRight, IconInbox, IconCalendar, IconCircleDot, IconUser, IconLink,
    IconFileText, IconDotsVertical, IconArrowLeft
  } from '@tabler/icons-svelte';

  const MAP: Record<string, any> = {
    microphone: IconMicrophone, graph: IconGraph, settings: IconSettings, search: IconSearch,
    plus: IconPlus, bolt: IconBolt, pin: IconPin, sun: IconSun, 'square-check': IconSquareCheck,
    'chevron-down': IconChevronDown, 'chevron-right': IconChevronRight, flame: IconFlame,
    folder: IconFolder, 'layout-sidebar': IconLayoutSidebar, adjustments: IconAdjustments,
    hash: IconHash, clock: IconClock, 'corner-down-right': IconCornerDownRight, inbox: IconInbox,
    calendar: IconCalendar, 'circle-dot': IconCircleDot, user: IconUser, link: IconLink,
    'file-text': IconFileText, 'dots-vertical': IconDotsVertical, 'arrow-left': IconArrowLeft
  };
  let { name, size = 16, stroke = 1.75 }: { name: string; size?: number; stroke?: number } = $props();
  const Cmp = $derived(MAP[name]);
</script>

{#if Cmp}<Cmp {size} {stroke} />{/if}
```

- [ ] **Step 2: Verify** — add `import GrIcon from '$lib/graphite/GrIcon.svelte'` to the gallery page temporarily and render `<GrIcon name="settings" />`; confirm a settings glyph appears. Remove the temporary line after.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/graphite/GrIcon.svelte
git commit -m "feat(graphite-web): Tabler icon wrapper (name->component map)"
```

### Task B3: GrButton

**Files:**
- Create: `web/src/lib/graphite/GrButton.svelte`

CSS spec (gr-shell.jsx): `.gr-headbtn` ghost = `bg:--raised; border:1px solid --line-2; color:--fg2; height:28px; padding:0 11px; border-radius:8px; font-size:12px; inline-flex; gap:6px` hover→`bg:--raised-2; color:--fg`. `.cta` = `bg:--coral; color:#10110f; border:transparent; font-weight:600`. Icon-only button uses `.gr-ic` = `30×30; border-radius:8px; color:--subtle; grid place-items:center` hover→`color:--fg; bg:--raised`.

- [ ] **Step 1: Write the component**

```svelte
<!-- web/src/lib/graphite/GrButton.svelte -->
<script lang="ts">
  import GrIcon from './GrIcon.svelte';
  let { variant = 'ghost', icon, children, ...rest }:
    { variant?: 'ghost' | 'cta'; icon?: string; children?: any } = $props();
  const iconOnly = $derived(icon && !children);
</script>

<button class="gr-btn" class:cta={variant === 'cta'} class:ic={iconOnly} {...rest}>
  {#if icon}<GrIcon name={icon} size={iconOnly ? 17 : 15} />{/if}
  {#if children}{@render children()}{/if}
</button>

<style>
  .gr-btn{height:28px;padding:0 11px;border-radius:8px;font-size:12px;display:inline-flex;
    align-items:center;gap:6px;white-space:nowrap;cursor:pointer;font-family:var(--sans);
    background:var(--raised);border:1px solid var(--line-2);color:var(--fg2);transition:all .14s;}
  .gr-btn:hover{background:var(--raised-2);color:var(--fg);}
  .gr-btn.cta{background:var(--coral);color:#10110f;border-color:transparent;font-weight:600;}
  .gr-btn.ic{width:30px;height:30px;padding:0;justify-content:center;background:transparent;
    border-color:transparent;color:var(--subtle);}
  .gr-btn.ic:hover{background:var(--raised);color:var(--fg);}
</style>
```

- [ ] **Step 2: Verify** in the gallery (`/g`): three buttons render — coral CTA, ghost, icon-only; hover changes bg/color. Commit.

```bash
git add web/src/lib/graphite/GrButton.svelte && git commit -m "feat(graphite-web): GrButton primitive"
```

### Task B4: GrChip

**Files:**
- Create: `web/src/lib/graphite/GrChip.svelte`

CSS: `.gr-chip` = `inline-flex; gap:6px; height:26px; padding:0 11px; border-radius:8px; bg:--raised; border:1px solid --line; color:--fg2; font-size:12px` hover→`border-color:--line-2`; `.active`→`bg:--coral-dim; border-color:--coral-line; color:--coral`. Count `.n` = `font-family:--mono; font-size:10px; color:--faint` (active→`--coral`).

- [ ] **Step 1: Write the component**

```svelte
<!-- web/src/lib/graphite/GrChip.svelte -->
<script lang="ts">
  let { active = false, count, children, ...rest }:
    { active?: boolean; count?: number; children?: any } = $props();
</script>

<button class="gr-chip" class:active {...rest}>
  {#if children}{@render children()}{/if}
  {#if count !== undefined}<span class="n">{count}</span>{/if}
</button>

<style>
  .gr-chip{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 11px;border-radius:8px;
    cursor:pointer;background:var(--raised);border:1px solid var(--line);color:var(--fg2);
    font-size:12px;font-family:var(--sans);transition:all .14s;}
  .gr-chip:hover{border-color:var(--line-2);}
  .gr-chip.active{background:var(--coral-dim);border-color:var(--coral-line);color:var(--coral);}
  .gr-chip .n{font-family:var(--mono);font-size:10px;color:var(--faint);}
  .gr-chip.active .n{color:var(--coral);}
</style>
```

- [ ] **Step 2: Verify** in `/g`: active chip is coral-tinted; count badge is mono + faint (coral when active). Commit.

```bash
git add web/src/lib/graphite/GrChip.svelte && git commit -m "feat(graphite-web): GrChip primitive"
```

### Task B5: GrTypeDot + GrTypeTag

**Files:**
- Create: `web/src/lib/graphite/GrTypeDot.svelte`
- Create: `web/src/lib/graphite/GrTypeTag.svelte`

`.gr-dot` = `6×6; border-radius:50%; background:var(--<type>)`. `.gr-typetag` = `inline-flex; gap:6px; height:21px; padding:0 9px; border-radius:6px; font-family:--mono; font-size:10.5px; background:--raised; border:1px solid --line-2; color:var(--<type>)` with a `.sw` swatch `6×6; border-radius:2px; background:var(--<type>)`.

- [ ] **Step 1: GrTypeDot**

```svelte
<!-- web/src/lib/graphite/GrTypeDot.svelte -->
<script lang="ts">
  let { type = 'note', size = 6 }: { type?: string; size?: number } = $props();
</script>
<span class="gr-dot" style="--d:{size}px;background:var(--{type});"></span>
<style>.gr-dot{width:var(--d);height:var(--d);border-radius:50%;flex-shrink:0;display:inline-block;}</style>
```

- [ ] **Step 2: GrTypeTag**

```svelte
<!-- web/src/lib/graphite/GrTypeTag.svelte -->
<script lang="ts">
  let { type = 'project', children }: { type?: string; children?: any } = $props();
</script>
<span class="gr-typetag" style="color:var(--{type});">
  <span class="sw" style="background:var(--{type});"></span>{#if children}{@render children()}{/if}
</span>
<style>
  .gr-typetag{display:inline-flex;align-items:center;gap:6px;height:21px;padding:0 9px;border-radius:6px;
    font-family:var(--mono);font-size:10.5px;letter-spacing:.02em;background:var(--raised);
    border:1px solid var(--line-2);}
  .gr-typetag .sw{width:6px;height:6px;border-radius:2px;flex-shrink:0;}
</style>
```

- [ ] **Step 3: Verify** in `/g`: six colored dots + two type tags with matching swatch colors. Commit.

```bash
git add web/src/lib/graphite/GrTypeDot.svelte web/src/lib/graphite/GrTypeTag.svelte
git commit -m "feat(graphite-web): GrTypeDot + GrTypeTag primitives"
```

### Task B6: GrRow

**Files:**
- Create: `web/src/lib/graphite/GrRow.svelte`

`.gr-row` = `flex; gap:9px; padding:6px 8px; border-radius:7px; color:--fg2; font-size:12.5px` hover→`bg:--raised-2`; `.active`→`bg:--raised-2; color:--fg`. `.ic` color `--subtle`; `.lb` flex+ellipsis; `.mt` = `font-family:--mono; font-size:10.5px; color:--faint; tabular-nums` (urgent→`--coral`).

- [ ] **Step 1: Write the component**

```svelte
<!-- web/src/lib/graphite/GrRow.svelte -->
<script lang="ts">
  import GrIcon from './GrIcon.svelte';
  let { icon, label, meta, urgent = false, active = false, ...rest }:
    { icon?: string; label?: string; meta?: string; urgent?: boolean; active?: boolean } = $props();
</script>
<div class="gr-row" class:active {...rest}>
  {#if icon}<span class="ic"><GrIcon name={icon} size={15} /></span>{/if}
  <span class="lb">{label}</span>
  {#if meta}<span class="mt" class:urg={urgent}>{meta}</span>{/if}
</div>
<style>
  .gr-row{display:flex;align-items:center;gap:9px;padding:6px 8px;border-radius:7px;cursor:pointer;
    color:var(--fg2);font-size:12.5px;transition:background .12s;}
  .gr-row:hover,.gr-row.active{background:var(--raised-2);}
  .gr-row.active{color:var(--fg);}
  .gr-row .ic{color:var(--subtle);display:flex;}
  .gr-row .lb{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
  .gr-row .mt{font-family:var(--mono);font-size:10.5px;color:var(--faint);
    font-variant-numeric:tabular-nums;white-space:nowrap;}
  .gr-row .mt.urg{color:var(--coral);}
</style>
```

- [ ] **Step 2: Verify** in `/g` (inside the widget): rows hover-highlight; the `urgent` row's meta is coral. Commit.

```bash
git add web/src/lib/graphite/GrRow.svelte && git commit -m "feat(graphite-web): GrRow primitive"
```

### Task B7: GrWidget (rail widget shell)

**Files:**
- Create: `web/src/lib/graphite/GrWidget.svelte`

`.gr-w` = `bg:--raised; border:1px solid --line; border-radius:11px; overflow:hidden`. `.gr-w-head` = `flex; gap:8px; padding:9px 11px 7px`; title `.ti` = `flex:1; font-size:11px; font-weight:600; letter-spacing:.04em; text-transform:uppercase; color:--fg2`; badge `.bd` = `font-family:--mono; font-size:10px; color:--subtle; bg:--bg; border:1px solid --line; border-radius:5px; padding:1px 6px`. `.gr-w-body` = `padding:2px 7px 9px`.

- [ ] **Step 1: Write the component** (this is the widget HOST shell — parity ships a fixed widget set; configurability is iterate-phase per the spec).

```svelte
<!-- web/src/lib/graphite/GrWidget.svelte -->
<script lang="ts">
  import GrIcon from './GrIcon.svelte';
  let { title, icon, badge, children }:
    { title: string; icon?: string; badge?: string; children?: any } = $props();
</script>
<section class="gr-w">
  <header class="gr-w-head">
    {#if icon}<span class="ic"><GrIcon name={icon} size={14} /></span>{/if}
    <span class="ti">{title}</span>
    {#if badge}<span class="bd">{badge}</span>{/if}
    <span class="caret"><GrIcon name="chevron-down" size={14} /></span>
  </header>
  <div class="gr-w-body">{#if children}{@render children()}{/if}</div>
</section>
<style>
  .gr-w{background:var(--raised);border:1px solid var(--line);border-radius:11px;overflow:hidden;}
  .gr-w-head{display:flex;align-items:center;gap:8px;padding:9px 11px 7px;}
  .gr-w-head .ic{color:var(--subtle);display:flex;}
  .gr-w-head .ti{flex:1;font-size:11px;font-weight:600;letter-spacing:.04em;text-transform:uppercase;color:var(--fg2);}
  .gr-w-head .bd{font-family:var(--mono);font-size:10px;color:var(--subtle);background:var(--bg);
    border:1px solid var(--line);border-radius:5px;padding:1px 6px;white-space:nowrap;}
  .gr-w-head .caret{color:var(--faint);margin-left:2px;display:flex;}
  .gr-w-body{padding:2px 7px 9px;}
</style>
```

- [ ] **Step 2: Verify** in `/g`: "TODAY" widget (uppercase mono-ish header, "3" badge, chevron) wraps the two rows. Commit.

```bash
git add web/src/lib/graphite/GrWidget.svelte && git commit -m "feat(graphite-web): GrWidget rail-widget shell"
```

### Task B8: Web foundation gate

- [ ] **Step 1: Run the build + type check**

Run: `cd web && pnpm exec svelte-check --threshold error 2>&1 | tail -20`
Expected: no NEW errors in `src/lib/graphite/` or `src/routes/g/` (1 pre-existing VoiceCaptureButton error is OK).

- [ ] **Step 2: Visual verification (Chrome DevTools MCP)** — navigate to `http://localhost:5176/g`, screenshot, confirm against `.docs/ai/design/graphite/screenshots/` that buttons/chips/dots/tags/rows/widget match the mockup's spacing + color. Note any drift; fix in the offending component.

- [ ] **Step 3: Commit any fixes** with `fix(graphite-web): …`.

---

## Part C — iOS foundation (new SwiftUI tree, reuse the Theme system)

> iOS already has `Sources/DesignSystem/` (`Theme` struct with `bg/bg2/bg3/bg4`, `line/lineSoft`, `fgDefault/fgMuted/fgSubtle/fgFaint`, `accentPrimary/Secondary/Spark`, `typeTask/Event/Note/Project/Person/Query/Template`), `@Environment(\.theme)`, `TypeScale`, `Density`, `Color+Hex`. The foundation REUSES all of it — adds a `.graphite` theme + new primitives. New primitives live under `Sources/Graphite/` so they're isolated from the old `Sources/Components/` + `Sources/Views/` (deleted at cutover).

### Task C1: `.graphite` Theme case

**Files:**
- Modify: `app/Tesela-iOS/Sources/DesignSystem/Theme.swift`

Mapping (Graphite token → existing `Theme` role; verify exact field names by reading `Theme.swift` first):
`bg=#0E1014` · `bg2=surface #14171D` · `bg3=raised #1A1E26` · `bg4=raised-2 #20242D` · `line=line-2 rgba(255,255,255,.12)` · `lineSoft=line rgba(255,255,255,.07)` · `fgDefault=#EDEFF2` · `fgMuted=#CBD0D9` · `fgSubtle=#8A909C` · `fgFaint=#646B78` · `accentPrimary=coral #FF6B5A` · `accentSecondary=project #7493E8` · `accentSpark=coral` · `typeTask=#E8697F` · `typeEvent=#62B8CE` · `typeNote=#E4AE66` · `typeProject=#7493E8` · `typePerson=#AE90E6` · `typeQuery=#85BC63` · `typeTemplate=person #AE90E6`.
(`muted #AAB0BB` and `raised-3 #272C37` have no existing role slot — use `fgMuted`/`bg4` as nearest; note for the iterate-phase if a 5th level is wanted.)

- [ ] **Step 1: Read Theme.swift** to confirm the `ThemeID` enum cases, the `Theme` initializer signature, and the exact role field names + `Color(hex:)` helper. Mirror an existing dark theme's construction (e.g. the `tokyoNight` case).

- [ ] **Step 2: Add the `.graphite` case** to `ThemeID` and its palette to the `Theme` factory, following the existing pattern exactly (same fields, `Color(hex:)` for opaque values; for the `rgba(255,255,255,.07/.12)` lines use `Color.white.opacity(0.07/0.12)`).

- [ ] **Step 3: Verify it compiles**

Run: `cd app/Tesela-iOS && xcodegen generate && xcodebuild -scheme Tesela -sdk iphonesimulator -configuration Debug -destination 'generic/platform=iOS Simulator' build 2>&1 | tail -5`
Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add app/Tesela-iOS/Sources/DesignSystem/Theme.swift
git commit -m "feat(graphite-ios): add .graphite Theme palette"
```

### Task C2: Graphite icon mapping

**Files:**
- Read: `app/Tesela-iOS/Sources/Components/Icon.swift` (learn how it renders today — SF Symbols vs other)
- Create: `app/Tesela-iOS/Sources/Graphite/GrIcon.swift`

- [ ] **Step 1: Read `Icon.swift`** to see the existing rendering approach + reuse it.

- [ ] **Step 2: Write `GrIcon`** — a `View` taking the same string names as the web `GrIcon` (`"settings"`, `"sun"`, `"square-check"`, …), mapping each to the closest SF Symbol (e.g. `settings→gearshape`, `sun→sun.max`, `square-check→checkmark.square`, `microphone→mic`, `pin→pin`, `bolt→bolt`, `graph→point.3.connected.trianglepath.dotted`, `inbox→tray`, `calendar→calendar`, `search→magnifyingglass`, `plus→plus`, `chevron-down→chevron.down`, `chevron-right→chevron.right`, `flame→flame`, `circle-dot→smallcircle.filled.circle`, `folder→folder`, `hash→number`, `clock→clock`, `link→link`, `file-text→doc.text`, `user→person`, `dots-vertical→ellipsis`, `arrow-left→chevron.left`, `corner-down-right→arrow.turn.down.right`, `layout-sidebar→sidebar.left`, `adjustments→slider.horizontal.3`). Take `name`, `size`, optional `weight`; render `Image(systemName:)` tinted by the caller. (Bundling Tabler SVGs for exact-icon parity is an iterate-phase option; SF Symbols are the parity baseline.)

- [ ] **Step 3: Verify** it compiles (folded into the C7 gate build). Commit.

```bash
git add app/Tesela-iOS/Sources/Graphite/GrIcon.swift
git commit -m "feat(graphite-ios): GrIcon SF Symbol mapping"
```

### Task C3: GrButton (SwiftUI)

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/GrButton.swift`

Mirror the web spec: ghost = `theme.bg3` fill, `theme.line` border, `theme.fgMuted` text, height 28, corner radius 8, h-pad 11, 12pt; cta = `theme.accentPrimary` fill, `#10110F` text, semibold. Read `@Environment(\.theme)` like existing components.

- [ ] **Step 1: Write the component** — a `Button` style or a `View` with `variant: GrButtonVariant { ghost, cta }` + optional `icon: String`. Use `theme.accentPrimary` / `theme.bg3` / `theme.line` / `theme.fgMuted` / `theme.fgDefault`; `.font(.tesela(.chip, density:))` or `.system(size:12, weight: variant == .cta ? .semibold : .regular)`; `RoundedRectangle(cornerRadius: 8)`. Icon-only variant = 30×30, `theme.fgSubtle`.

- [ ] **Step 2: Verify** in the C7 gallery + build. Commit.

```bash
git add app/Tesela-iOS/Sources/Graphite/GrButton.swift && git commit -m "feat(graphite-ios): GrButton primitive"
```

### Task C4: GrChip + GrTypeTag + GrTypeDot (SwiftUI)

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/GrChip.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/GrTypeTag.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/GrTypeDot.swift`

- [ ] **Step 1: GrTypeDot** — `Circle().fill(theme.typeColor(forKind:))` 6×6 (reuse the existing `Theme.typeColor(forKind:)` helper found in Task C1's read). Take a `kind`/`type` string or the app's `Block.Kind` enum.
- [ ] **Step 2: GrChip** — capsule-ish rounded rect (radius 8, h 26, h-pad 11): inactive `theme.bg3` + `theme.lineSoft` border + `theme.fgMuted`; active `theme.accentPrimary.opacity(0.14)` fill + `theme.accentPrimary.opacity(0.40)` border + `theme.accentPrimary` text; optional mono count badge (`.font(.system(size:10, design:.monospaced))`, `theme.fgFaint` / accent when active).
- [ ] **Step 3: GrTypeTag** — rounded rect (radius 6, h 21, h-pad 9): `theme.bg3` fill, `theme.line` border, `theme.typeColor(forKind:)` text, leading 6×6 rounded swatch in the type color, mono 10.5pt label.
- [ ] **Step 4: Verify** in the C7 gallery + build. Commit.

```bash
git add app/Tesela-iOS/Sources/Graphite/GrChip.swift app/Tesela-iOS/Sources/Graphite/GrTypeTag.swift app/Tesela-iOS/Sources/Graphite/GrTypeDot.swift
git commit -m "feat(graphite-ios): GrChip + GrTypeTag + GrTypeDot primitives"
```

### Task C5: GrRow (SwiftUI)

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/GrRow.swift`

- [ ] **Step 1: Write the component** — `HStack(spacing:9)` of optional leading `GrIcon` (`theme.fgSubtle`), a label (`theme.fgMuted`, 12.5pt, `.lineLimit(1)`), and an optional trailing mono meta (`theme.fgFaint`, 10.5pt, monospaced; `theme.accentPrimary` when `urgent`). Padding `6/8`, radius 7; `active`/pressed fill `theme.bg4`.

- [ ] **Step 2: Verify** in the C7 gallery + build. Commit.

```bash
git add app/Tesela-iOS/Sources/Graphite/GrRow.swift && git commit -m "feat(graphite-ios): GrRow primitive"
```

### Task C6: GrWidget (SwiftUI rail/library widget shell)

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/GrWidget.swift`

- [ ] **Step 1: Write the component** — a container `VStack(spacing:0)`: header `HStack` (optional `GrIcon` `theme.fgSubtle`; uppercased 11pt-semibold `theme.fgMuted` title with `.tracking(0.4)`; optional mono badge pill `theme.bg` fill + `theme.line` border; trailing chevron `theme.fgFaint`), body slot. Card = `theme.bg3` fill + `theme.line` border, radius 11, clipped. (On mobile this is the Library widget-grid cell per the spec; same shell.)

- [ ] **Step 2: Verify** in the C7 gallery + build. Commit.

```bash
git add app/Tesela-iOS/Sources/Graphite/GrWidget.swift && git commit -m "feat(graphite-ios): GrWidget shell"
```

### Task C7: iOS foundation gate — primitives gallery + build

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/GrGalleryView.swift`
- Modify: `app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift` (add a temporary dev NavigationLink to the gallery — removed at cutover) OR add a `#Preview` to the gallery file.

- [ ] **Step 1: Write `GrGalleryView`** — a `ScrollView` wrapping the app in `.environment(\.theme, Theme.graphite)` (or via the existing `TeselaAppearance`), rendering all primitives (buttons, chips, dots row, type tags, a GrWidget containing GrRows) — the iOS analogue of the web `/g` page. Add a `#Preview { GrGalleryView().environment(\.theme, .graphite) }` so it renders in Xcode canvas without wiring navigation.

- [ ] **Step 2: Build**

Run: `cd app/Tesela-iOS && xcodegen generate && xcodebuild -scheme Tesela -sdk iphonesimulator -configuration Debug -destination 'generic/platform=iOS Simulator' build 2>&1 | tail -5`
Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Visual verification** — boot the sim, navigate to the gallery (or screenshot the Xcode preview), confirm primitives match the Graphite mockup colors/spacing. Note drift; fix in the offending component.

- [ ] **Step 4: Commit**

```bash
git add app/Tesela-iOS/Sources/Graphite/GrGalleryView.swift app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift
git commit -m "feat(graphite-ios): primitives gallery + foundation gate"
```

---

## Foundation done — exit criteria

- Web: `/g` renders the primitives gallery on the Graphite surface; `svelte-check` clean; primitives use `var(--*)` only (no hardcoded hex) → theme-swappable.
- iOS: `xcodebuild` SUCCEEDED; gallery preview renders the primitives via `Theme.graphite` + `@Environment(\.theme)` → theme-swappable.
- Both: token VALUES trace to `.docs/ai/design/graphite/tokens.json`; primitives mirror the same names/specs cross-platform; old UI untouched.
- **Next plan:** Shell phase — web topbar + widget-rail host + panes container + status line + ⌘K palette + leader overlay; iOS tab bar + header + capture sheet + nav. (Separate plan, authored after this lands.)

---

## Self-review (against the spec)

- **Spec foundation step** ("tokens (web CSS vars + iOS Swift theme from one source), Tabler icon set, primitives (buttons, chips, rows, widget shell, type tags/dots)") → covered: A1 (source) + B1/C1 (tokens) + B2/C2 (icons) + B3–B7/C3–C6 (primitives). ✓
- **Locked decision 1** (clean SvelteKit rebuild, new tree) → B1 fresh `/g` tree, scoped tokens, no app.css role-token reuse. ✓
- **Locked decision 3** (iOS reuses Loro FFI/MosaicService; new view layer) → Part C touches only `DesignSystem/` + new `Graphite/`; Data/Generated/Sync untouched. ✓
- **Locked decision 6** (rail = widget host; parity = fixed set) → B7/C6 are the shell; configurability explicitly deferred. ✓
- **"Tokens theme-indirection-ready, no hardcoded hex in components"** → web components use `var(--*)`; iOS use `theme.*` — both swappable. ✓
- **Reuse-vs-rebuild boundary** → only presentational primitives are new; vetted lib logic + FFI referenced, never edited. ✓
- **Type consistency** → web component names (`GrButton`/`GrChip`/`GrTypeDot`/`GrTypeTag`/`GrRow`/`GrWidget`/`GrIcon`) mirror iOS (`GrButton`/…); icon string names shared across `GrIcon` on both platforms. ✓
- **Open decision (web surface)** resolved per spec lean: fresh route tree (`/g`) in the existing app, not a separate toolchain. ✓
