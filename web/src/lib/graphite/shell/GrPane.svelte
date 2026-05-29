<!-- web/src/lib/graphite/shell/GrPane.svelte -->
<script lang="ts">
  /*
   * Graphite first-class pane container. Splits-ready: the `variant` prop
   * sets the `.focus` / `.side` flex ratios from the mockup so a future
   * split-management phase can compose two panes side-by-side. The body
   * is a slot — the shell passes a placeholder this phase; real views fill
   * it next. Title/subtitle/meta are props so the shell can drive them from
   * the focused buffer.
   */
  import GrIcon from '$lib/graphite/GrIcon.svelte';

  let {
    title,
    subtitle,
    meta,
    canBack = false,
    variant,
    onback,
    actions,
    children,
  }: {
    title: string;
    subtitle?: string;
    meta?: string;
    canBack?: boolean;
    variant?: 'focus' | 'side';
    onback?: () => void;
    actions?: any;
    children?: any;
  } = $props();
</script>

<div class="gr-pane" class:focus={variant === 'focus'} class:side={variant === 'side'}>
  <div class="gr-pane-head">
    {#if canBack}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="gr-back" onclick={() => onback?.()}>
        <GrIcon name="arrow-left" size={16} />
      </span>
    {/if}
    <span class="ttl">{title}</span>
    {#if subtitle}<span class="sub">{subtitle}</span>{/if}
    <span class="sp"></span>
    {#if meta}<span class="meta">{meta}</span>{/if}
    {#if actions}{@render actions()}{/if}
  </div>
  <div class="gr-pane-body">
    {#if children}{@render children()}{/if}
  </div>
</div>

<style>
  .gr-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  .gr-pane.focus {
    flex: 1.7;
  }
  .gr-pane.side {
    flex: 1;
    background: var(--surface);
    border-left: 1px solid var(--line);
    max-width: 420px;
  }
  .gr-pane-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 14px 18px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .gr-pane-head .gr-back {
    color: var(--subtle);
    cursor: pointer;
    display: flex;
  }
  .gr-pane-head .ttl {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--fg);
    white-space: nowrap;
  }
  .gr-pane-head .sub {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
  .gr-pane-head .sp {
    flex: 1;
  }
  .gr-pane-head .meta {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .gr-pane-body {
    flex: 1;
    overflow: auto;
    padding: 14px 18px;
  }
</style>
