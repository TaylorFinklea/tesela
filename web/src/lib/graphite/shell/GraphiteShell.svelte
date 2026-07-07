<!-- web/src/lib/graphite/shell/GraphiteShell.svelte -->
<script lang="ts">
  /*
   * Graphite shell root. Composes the chrome — top bar / (rail + main with
   * one pane) / status — and mounts the ⌘K palette + Space leader overlays.
   *
   * Behavior is 100% reused. The capture-phase keydown listener below mirrors
   * web/src/routes/v4/+layout.svelte's wiring (the only place behavior
   * connects today): Space → openLeader(), ⌘K → openStation(), `:` →
   * openColonMode(), all behind the same text-entry guard (`isTextEntry`) and
   * the same plain-input exception for `:` so colons can still be typed into
   * settings fields. Only the markup + CSS are new.
   *
   * The `.gr-root` scope + tokens are provided by /g/+layout.svelte
   * (the foundation), so this component renders inside that.
   *
   * The main area renders the active tab's binary pane tree via
   * <GrLayoutTree> (so vsplit/hsplit show up); each leaf routes its buffer
   * to a Graphite view in GrLeaf. The shell keeps the single focused-page
   * Loro doc (it follows focus) + the keydown wiring (leader / ⌘K / `:` /
   * Ctrl-W pane motion).
   */
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { flushAllOutbound } from '$lib/loro/note-doc-registry.svelte';
  import GrTopBar from './GrTopBar.svelte';
  import GrRail from './GrRail.svelte';
  import GrStatus from './GrStatus.svelte';
  import GrCommandPalette from './GrCommandPalette.svelte';
  import GrLeaderOverlay from './GrLeaderOverlay.svelte';
  import GrLayoutTree from './GrLayoutTree.svelte';
  import {
    getFocusedBuffer,
    getFocusedLeafId,
    getActiveTab,
    moveFocus,
    openPageInFocused,
  } from '$lib/buffer/state.svelte';
  import { asPageId } from '$lib/buffer/types';
  import { openStation } from '$lib/stores/station.svelte';
  import { openLeader } from '$lib/leader/leader-tree.svelte';
  import { openColonMode } from '$lib/stores/colon-mode.svelte';
  import { getVimMode } from '$lib/stores/pane-state.svelte';
  import { openSettingsOverlay } from '$lib/stores/fullscreen-overlay.svelte';
  import { getFocusedBlock } from '$lib/stores/current-block.svelte';
  import { isEditorFocused } from '$lib/stores/focused-editor.svelte';
  import { resolveShortcut, type CommandContext } from '$lib/command-registry.svelte';
  import * as keybindings from '$lib/stores/keybindings.svelte';
  import ColonCommandLine from '$lib/components/shell/ColonCommandLine.svelte';
  import FullscreenOverlay from '$lib/components/shell/FullscreenOverlay.svelte';
  import PeekPopover from '$lib/components/shell/PeekPopover.svelte';

  const focusedBuffer = $derived(getFocusedBuffer());
  const focusedLeafId = $derived(getFocusedLeafId());

  // The active tab's pane tree + a shared drag flag for the resizers.
  const tab = $derived(getActiveTab());

  // Command context drives context-aware filtering in the unified registry.
  const commandCtx = $derived<CommandContext>({
    route: $page.route?.id,
    bufferKind: focusedBuffer?.kind ?? null,
    vimMode: getVimMode(),
    focusedBlock: (() => {
      const b = getFocusedBlock();
      return b ? { id: b.id, properties: b.properties ?? {} } : null;
    })(),
    splitOpen: tab?.layout?.type === 'split',
    // Lets the leader admit editor commands (i/p buckets) only when a block is
    // focused; they run via tesela:run-editor-command on the focused editor.
    editorFocused: isEditorFocused(),
  });
  const dragRef = $state({ value: false });
  // Only show the per-pane focus accent when the layout is actually split —
  // a lone pane renders exactly as the pre-split shell did.
  const isSplit = $derived(tab?.layout.type === 'split');

  // A daily page is either the default (empty pageId) leaf or a page whose
  // id is a YYYY-MM-DD date — those render as the continuous JournalView.
  function isDailyPageId(pageId: string): boolean {
    return pageId === '' || /^\d{4}-\d{2}-\d{2}$/.test(pageId);
  }

  // Status-bar path for the focused buffer (the per-pane heads live in the
  // Gr* views / GrLeaf now).
  const paneTitle = $derived.by(() => {
    const b = focusedBuffer;
    if (!b) return 'Journal';
    if (b.kind === 'page') return isDailyPageId(b.pageId) ? 'Journal' : (b.pageId || 'Untitled page');
    if (b.kind === 'derived') return b.rendererName;
    if (b.kind === 'ambient') return b.ambientName;
    return 'Graphite';
  });

  // tesela-baa: per-note Loro docs are acquired by each mounted BlockOutliner
  // (ref-counted registry) — the shell no longer opens a doc for the focused
  // buffer. On teardown, force any pending rAF-coalesced deltas onto the wire
  // so the last keystrokes before a shell swap still ship.
  onDestroy(() => {
    flushAllOutbound();
  });

  // ── keydown wiring (mirror of v4/+layout.svelte) ─────────────────────────
  function isTextEntry(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el) return false;
    return (
      el.tagName === 'INPUT' ||
      el.tagName === 'TEXTAREA' ||
      el.isContentEditable ||
      !!el.closest?.('.cm-editor')
    );
  }

  // Ctrl-W h/j/k/l prefix for vim-style window motion (mirror of v4). Moving
  // focus by keyboard is what makes a split usable hands-on the keyboard.
  let awaitingCtrlW = false;
  let ctrlWTimer: ReturnType<typeof setTimeout> | null = null;
  function clearCtrlW() {
    awaitingCtrlW = false;
    if (ctrlWTimer) {
      clearTimeout(ctrlWTimer);
      ctrlWTimer = null;
    }
  }

  // Pane-motion chords must not steal keys from active text entry: vim
  // insert-mode i_CTRL-W (delete word back) and plain inputs (palette,
  // settings fields) keep Ctrl-W. Only a cm-editor in non-INSERT mode (or
  // no text entry at all) arms the prefix — same gating the `:` handler
  // below uses.
  function inActiveTextEntry(target: EventTarget | null): boolean {
    const t = target as HTMLElement | null;
    const inCmEditor = !!t?.closest?.('.cm-editor');
    const inPlainEntry =
      !inCmEditor &&
      (t?.tagName === 'INPUT' ||
        t?.tagName === 'TEXTAREA' ||
        !!t?.isContentEditable);
    const inInsertMode = inCmEditor && getVimMode() === 'INSERT';
    return inPlainEntry || inInsertMode;
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey;

      // Ctrl-W then h/j/k/l moves focus between split panes.
      if (
        !e.metaKey &&
        !e.altKey &&
        !e.shiftKey &&
        e.ctrlKey &&
        (e.key === 'w' || e.key === 'W') &&
        !inActiveTextEntry(e.target)
      ) {
        e.preventDefault();
        e.stopPropagation();
        awaitingCtrlW = true;
        if (ctrlWTimer) clearTimeout(ctrlWTimer);
        ctrlWTimer = setTimeout(clearCtrlW, 1500);
        return;
      }
      if (awaitingCtrlW) {
        const k = e.key.toLowerCase();
        if (
          (k === 'h' || k === 'j' || k === 'k' || k === 'l') &&
          !inActiveTextEntry(e.target)
        ) {
          clearCtrlW();
          e.preventDefault();
          e.stopPropagation();
          const dir =
            k === 'h' ? 'left' : k === 'l' ? 'right' : k === 'j' ? 'down' : 'up';
          moveFocus(dir);
          return;
        }
        clearCtrlW();
      }

      // `:` opens ex-mode (colon command line), even inside a cm-editor —
      // but let plain HTML inputs/textareas/contenteditables keep the colon.
      if (!mod && !e.altKey && !e.ctrlKey && e.key === ':') {
        const t = e.target as HTMLElement | null;
        const inCmEditor = !!t?.closest?.('.cm-editor');
        const inPlainEntry =
          !inCmEditor &&
          (t?.tagName === 'INPUT' ||
            t?.tagName === 'TEXTAREA' ||
            !!t?.isContentEditable);
        // In a cm-editor's INSERT mode, `:` must type a literal colon (so the
        // user can write `key:: value`); only NORMAL mode opens ex-mode.
        const inInsertMode = inCmEditor && getVimMode() === 'INSERT';
        if (!inPlainEntry && !inInsertMode) {
          e.preventDefault();
          e.stopPropagation();
          openColonMode({ priorPaneId: getFocusedLeafId() as unknown as string | undefined });
          return;
        }
      }

      // Space opens the leader chord menu when NOT in a text entry.
      if (
        !mod &&
        !e.altKey &&
        !e.ctrlKey &&
        e.key === ' ' &&
        !isTextEntry(e.target)
      ) {
        e.preventDefault();
        e.stopPropagation();
        openLeader();
        return;
      }

      // ── registry-driven ⌘-shortcut ladder ────────────────────────────────
      // resolveShortcut skips BROWSER_RESERVED_KEYS (⌘W/⌘T/etc) and only
      // returns commands whose when() passes for the current ctx. Reading
      // keybindings.snapshot() here (not closed-over) picks up live rebinds.
      const cmd = resolveShortcut(e, commandCtx, keybindings.snapshot());
      if (cmd) {
        e.preventDefault();
        void cmd.run(undefined, commandCtx);
        return;
      }
    };
    document.addEventListener('keydown', onKey, true);

    // Mirror v4: the journal's cm-vim <Space> action fires `tesela:leader`
    // at document level — catch it so the leader still opens from inside the
    // editor.
    const onLeaderEvent = () => openLeader();
    document.addEventListener('tesela:leader', onLeaderEvent);

    // BlockEditor's vim NORMAL-mode `g` binding dispatches this to open the
    // leader pre-descended into "go to…". The original listener lived in the
    // legacy root layout (deleted 2026-05-15) — without one here, `g` in
    // NORMAL mode was a silent no-op in every /g block editor.
    const onOpenLeaderAt = (e: Event) => {
      const detail = (e as CustomEvent).detail as { path?: string[] } | null;
      openLeader(detail?.path ?? []);
    };
    document.addEventListener('tesela:open-leader-at', onOpenLeaderAt);

    // The desktop (Tauri) native menu — Settings (⌘,) — dispatches this so the
    // app's own settings overlay opens. (In the browser, ⌘K / the gear / leader
    // `,` already open it.)
    const onOpenSettings = () => openSettingsOverlay('general');
    document.addEventListener('tesela:open-settings', onOpenSettings);

    // Mirror v4 (+layout:342): PageTagsChips' chip clicks fire
    // `tesela:open-tag` with `{ value: <slug> }`. Open the tag's page in the
    // focused buffer, same as the v5 `open-tag` NavigationIntent.
    const onOpenTag = (e: Event) => {
      const detail = (e as CustomEvent).detail as { value?: string } | null;
      const value = detail?.value;
      if (!value) return;
      openPageInFocused(asPageId(value.toLowerCase()));
    };
    document.addEventListener('tesela:open-tag', onOpenTag as EventListener);

    return () => {
      document.removeEventListener('keydown', onKey, true);
      document.removeEventListener('tesela:leader', onLeaderEvent);
      document.removeEventListener('tesela:open-leader-at', onOpenLeaderAt);
      document.removeEventListener('tesela:open-settings', onOpenSettings);
      document.removeEventListener('tesela:open-tag', onOpenTag as EventListener);
    };
  });
</script>

<div class="gr-shell">
  <GrTopBar />

  <div class="gr-body">
    <GrRail />
    <div class="gr-main" class:dragging={dragRef.value}>
      {#if tab}
        <GrLayoutTree
          node={tab.layout}
          focusedLeafId={focusedLeafId}
          activeDragRef={dragRef}
          showFocus={isSplit}
        />
      {/if}
    </div>
  </div>

  <GrStatus path={paneTitle} />

  <GrCommandPalette ctx={commandCtx} />
  <GrLeaderOverlay ctx={commandCtx} />
  <ColonCommandLine ctx={commandCtx} />
  <!-- Settings / graph fullscreen overlays (the gear, ⌘G, leader `,`, and the
       desktop Settings menu all drive these via the overlay store). Was only
       mounted on /v4 — so on /g they set the store but never rendered. -->
  <FullscreenOverlay />
  <!-- Peek popover (⌘I, leader `p`, `:peek`). Same was-only-on-/v4 disease as
       FullscreenOverlay: the store flipped but nothing rendered on /g. -->
  <PeekPopover />
</div>

<style>
  .gr-shell {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-rows: 48px 1fr 30px;
    overflow: hidden;
  }
  .gr-body {
    display: flex;
    min-height: 0;
    overflow: hidden;
    position: relative;
    flex: 1;
  }
  .gr-main {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
  }
  /* The root leaf or split fills the main area (mirror /v4's .v4-grid rule). */
  .gr-main > :global(.gr-leaf),
  .gr-main > :global(.gr-split) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .gr-main.dragging {
    user-select: none;
  }
  .gr-main.dragging :global(.cm-editor) {
    pointer-events: none;
  }
</style>
