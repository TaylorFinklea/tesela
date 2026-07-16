<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    ReleaseNotesSeenState,
    loadBundledReleaseNotes,
    platformReleaseHistory,
    releaseDateLabel,
    releaseVersionLabel,
    resolveReleasePlatform,
    type ReleaseNote,
  } from "$lib/release-notes";
  import {
    closeOverlay,
    takeReleaseNotesReturnFocus,
  } from "$lib/stores/fullscreen-overlay.svelte";

  type Mode = "current" | "history" | "detail";

  const platform = resolveReleasePlatform();
  const catalog = loadBundledReleaseNotes();
  const history = catalog ? platformReleaseHistory(catalog, platform) : [];
  const current = history[0] ?? null;
  const older = history.slice(1);

  let mode = $state<Mode>("current");
  let selected = $state<ReleaseNote | null>(null);
  let focusTarget = $state<HTMLElement | undefined>();
  let previousFocus: HTMLElement | null = null;

  const visibleRelease = $derived(mode === "detail" ? selected : current);

  async function focusSurface() {
    await tick();
    focusTarget?.focus();
  }

  function showHistory() {
    mode = "history";
    selected = null;
    void focusSurface();
  }

  function showOlder(release: ReleaseNote) {
    selected = release;
    mode = "detail";
    void focusSurface();
  }

  function goBack() {
    if (mode === "detail") {
      showHistory();
      return;
    }
    mode = "current";
    selected = null;
    void focusSurface();
  }

  onMount(() => {
    previousFocus = takeReleaseNotesReturnFocus();
    if (catalog && current) {
      new ReleaseNotesSeenState(catalog, platform, localStorage).markCurrentRendered();
    }
    void focusSurface();

    return () => {
      requestAnimationFrame(() => {
        if (previousFocus?.isConnected) {
          previousFocus.focus();
          return;
        }
        document.querySelector<HTMLElement>("[data-release-notes-entry-button]")?.focus();
      });
    };
  });
</script>

<div class="release-notes" role="dialog" aria-modal="true" aria-labelledby="release-notes-title">
  <header class="release-head">
    <div class="release-head-side">
      {#if mode !== "current"}
        <button class="text-button" type="button" onclick={goBack} aria-label="Back">
          ← Back
        </button>
      {:else}
        <span class="eyebrow">Tesela</span>
      {/if}
    </div>
    <span class="release-head-title">What’s New</span>
    <div class="release-head-side end">
      <button class="done-button" type="button" onclick={closeOverlay}>Done</button>
    </div>
  </header>

  {#if !catalog || !current}
    <main class="release-scroll centered" tabindex="-1" bind:this={focusTarget}>
      <div class="empty-card">
        <span class="spark" aria-hidden="true">✦</span>
        <h1 id="release-notes-title">Release notes unavailable</h1>
        <p>Tesela is ready to use. Try opening What’s New again after the next update.</p>
        <button class="primary-button" type="button" onclick={closeOverlay}>Done</button>
      </div>
    </main>
  {:else if mode === "history"}
    <main class="release-scroll" tabindex="-1" bind:this={focusTarget}>
      <div class="history-wrap">
        <p class="eyebrow">Release history</p>
        <h1 id="release-notes-title">Earlier releases</h1>
        <p class="lede">The changes that led to the version you’re using now.</p>
        <ul class="history-list">
          {#each older as release (release.id)}
            <li>
              <button
                class="history-row"
                type="button"
                onclick={() => showOlder(release)}
              >
                <span class="history-meta">
                  {releaseVersionLabel(release, platform)} · {releaseDateLabel(release)}
                </span>
                <span class="history-title">{release.title}</span>
                <span class="history-summary">{release.summary}</span>
                <span class="history-arrow" aria-hidden="true">→</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    </main>
  {:else if visibleRelease}
    <main
      class="release-scroll"
      tabindex="-1"
      bind:this={focusTarget}
      data-release-notes-detail={visibleRelease.id}
    >
      <article class="detail">
        <div class="hero-mark" aria-hidden="true">
          <span>✦</span>
        </div>
        <p class="release-meta">
          {releaseVersionLabel(visibleRelease, platform)} · {releaseDateLabel(visibleRelease)}
        </p>
        <h1 id="release-notes-title">{visibleRelease.title}</h1>
        <p class="lede">{visibleRelease.summary}</p>

        <div class="change-groups">
          {#if visibleRelease.new.length > 0}
            <section class="change-group">
              <div class="group-heading"><span class="group-icon new" aria-hidden="true">＋</span><h2>New</h2></div>
              <ul>{#each visibleRelease.new as item}<li>{item}</li>{/each}</ul>
            </section>
          {/if}
          {#if visibleRelease.fixed.length > 0}
            <section class="change-group">
              <div class="group-heading"><span class="group-icon fixed" aria-hidden="true">✓</span><h2>Fixed</h2></div>
              <ul>{#each visibleRelease.fixed as item}<li>{item}</li>{/each}</ul>
            </section>
          {/if}
          {#if visibleRelease.important.length > 0}
            <section class="change-group important">
              <div class="group-heading"><span class="group-icon warning" aria-hidden="true">!</span><h2>Important</h2></div>
              <ul>{#each visibleRelease.important as item}<li>{item}</li>{/each}</ul>
            </section>
          {/if}
        </div>

        {#if mode === "current" && older.length > 0}
          <button class="history-button" type="button" onclick={showHistory}>
            <span>
              <strong>View older releases</strong>
              <small>{older.length} earlier {older.length === 1 ? "release" : "releases"}</small>
            </span>
            <span aria-hidden="true">→</span>
          </button>
        {/if}
      </article>
    </main>
  {/if}
</div>

<style>
  .release-notes {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr);
    color: var(--fg-default);
    background:
      radial-gradient(circle at 50% -16%, color-mix(in srgb, var(--accent-spark) 10%, transparent), transparent 42%),
      var(--bg);
  }
  .release-head {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 0 18px;
    border-bottom: 1px solid var(--line-soft);
    background: color-mix(in srgb, var(--bg) 90%, transparent);
  }
  .release-head-side { display: flex; align-items: center; }
  .release-head-side.end { justify-content: flex-end; }
  .release-head-title, .eyebrow, .release-meta, .history-meta {
    font-family: var(--theme-font-mono);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .release-head-title { font-size: 10px; color: var(--fg-subtle); }
  .eyebrow { font-size: 10px; color: var(--accent-spark); }
  button { font: inherit; }
  .text-button, .done-button {
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    padding: 6px 9px;
    font-size: 12px;
  }
  .text-button:hover, .done-button:hover { background: var(--bg-2); color: var(--fg-default); }
  .done-button { color: var(--accent-spark); font-weight: 600; }
  .release-scroll { overflow-y: auto; outline: none; }
  .release-scroll:focus-visible { box-shadow: inset 0 0 0 1px var(--accent-spark); }
  .detail, .history-wrap { width: min(680px, calc(100% - 40px)); margin: 0 auto; padding: 68px 0 80px; }
  .hero-mark {
    width: 54px;
    height: 54px;
    display: grid;
    place-items: center;
    margin-bottom: 22px;
    border: 1px solid color-mix(in srgb, var(--accent-spark) 45%, var(--line-soft));
    border-radius: 17px;
    color: var(--accent-spark);
    background: color-mix(in srgb, var(--accent-spark) 10%, var(--bg-2));
    font-size: 22px;
    box-shadow: 0 18px 48px color-mix(in srgb, var(--accent-spark) 8%, transparent);
  }
  .release-meta, .history-meta { color: var(--fg-faint); font-size: 10px; }
  h1 { margin: 9px 0 10px; font-size: clamp(30px, 5vw, 48px); line-height: 1.04; letter-spacing: -0.035em; }
  .lede { max-width: 580px; margin: 0; color: var(--fg-muted); font-size: 16px; line-height: 1.6; }
  .change-groups { display: grid; gap: 14px; margin-top: 40px; }
  .change-group {
    padding: 20px 22px;
    border: 1px solid var(--line-soft);
    border-radius: 14px;
    background: color-mix(in srgb, var(--bg-2) 76%, transparent);
  }
  .change-group.important {
    border-color: color-mix(in srgb, #e7ad62 42%, var(--line-soft));
    background: color-mix(in srgb, #e7ad62 7%, var(--bg-2));
  }
  .group-heading { display: flex; align-items: center; gap: 10px; }
  .group-heading h2 { margin: 0; font-size: 13px; letter-spacing: 0.01em; }
  .group-icon { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 7px; font-weight: 700; }
  .group-icon.new { color: #79c69b; background: color-mix(in srgb, #79c69b 14%, transparent); }
  .group-icon.fixed { color: #79aee9; background: color-mix(in srgb, #79aee9 14%, transparent); }
  .group-icon.warning { color: #e7ad62; background: color-mix(in srgb, #e7ad62 14%, transparent); }
  ul { margin: 14px 0 0 32px; padding: 0; color: var(--fg-muted); }
  li { padding: 4px 0; line-height: 1.55; }
  .history-button {
    width: 100%;
    margin-top: 22px;
    padding: 17px 18px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid var(--line-soft);
    border-radius: 13px;
    background: transparent;
    color: var(--fg-default);
    text-align: left;
    cursor: pointer;
  }
  .history-button:hover { border-color: color-mix(in srgb, var(--accent-spark) 45%, var(--line-soft)); background: var(--bg-2); }
  .history-button strong, .history-button small { display: block; }
  .history-button strong { font-size: 13px; }
  .history-button small { margin-top: 3px; color: var(--fg-faint); font-size: 10px; }
  .history-wrap h1 { margin-bottom: 8px; }
  .history-list { display: grid; gap: 9px; margin: 32px 0 0; padding: 0; list-style: none; }
  .history-list > li { padding: 0; }
  .history-row {
    position: relative;
    width: 100%;
    display: grid;
    gap: 5px;
    padding: 18px 48px 18px 18px;
    border: 1px solid var(--line-soft);
    border-radius: 13px;
    background: color-mix(in srgb, var(--bg-2) 70%, transparent);
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .history-row:hover { border-color: color-mix(in srgb, var(--accent-spark) 40%, var(--line-soft)); }
  .history-title { margin-top: 3px; font-size: 15px; font-weight: 650; }
  .history-summary { color: var(--fg-muted); font-size: 12px; line-height: 1.45; }
  .history-arrow { position: absolute; right: 18px; top: 50%; translate: 0 -50%; color: var(--fg-faint); }
  .centered { display: grid; place-items: center; padding: 30px; }
  .empty-card { width: min(440px, 100%); text-align: center; }
  .empty-card h1 { font-size: 30px; }
  .empty-card p { color: var(--fg-muted); line-height: 1.55; }
  .spark { color: var(--accent-spark); font-size: 26px; }
  .primary-button { margin-top: 14px; padding: 9px 18px; border: 0; border-radius: 9px; background: var(--accent-spark); color: var(--bg); cursor: pointer; font-weight: 650; }
  @media (max-width: 640px) {
    .release-head { padding: 0 10px; }
    .detail, .history-wrap { width: min(100% - 28px, 680px); padding-top: 44px; }
    .change-group { padding: 17px; }
  }
</style>
