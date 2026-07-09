<!-- web/src/lib/graphite/shell/GrVoicePopover.svelte -->
<script lang="ts">
  /*
   * Live dictation panel, anchored under the top bar's mic button.
   * Rendered whenever a voice session is non-idle (GrTopBar gates on
   * voiceOpen()). Shows committed text solid and the tentative tail
   * dimmed; non-streaming models get a listening hint instead of
   * partials. Esc cancels, Enter (or the mic/Done button) finishes.
   */
  import {
    voicePhase,
    voiceLive,
    voiceCommitted,
    voiceTentative,
    voiceErrorMessage,
    voiceElapsedSeconds,
    stopVoiceCapture,
    cancelVoiceCapture,
    dismissVoiceError,
  } from '$lib/voice/voice-capture.svelte';
  import { formatElapsed } from '$lib/voice/protocol';

  const phase = $derived(voicePhase());
  const hasText = $derived(voiceCommitted().length > 0 || voiceTentative().length > 0);

  /** True when the key event is being typed into an editor / input, so
   *  dictation shortcuts must not steal it. */
  function isEditableTarget(e: KeyboardEvent): boolean {
    const el = e.target as HTMLElement | null;
    if (!el) return false;
    const tag = el.tagName;
    return (
      tag === 'INPUT' ||
      tag === 'TEXTAREA' ||
      el.isContentEditable ||
      el.closest('.cm-editor') !== null
    );
  }

  /* Escape is handled window-wide (a common "get me out" affordance),
   * but never while a keystroke is going into an editor and never
   * during finalizing (that would throw away a transcript the server
   * is still sending). Enter-to-finish is intentionally NOT global —
   * pressing Enter in the editor while a note dictates must not submit
   * the session; the Done button covers finishing. */
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || isEditableTarget(e)) return;
    if (phase === 'error') {
      e.preventDefault();
      dismissVoiceError();
    } else if (phase === 'starting' || phase === 'listening') {
      e.preventDefault();
      cancelVoiceCapture();
    }
    // finalizing: let Escape fall through — don't discard a pending
    // transcript.
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="gr-voice" role="status" aria-live="polite">
  <div class="head">
    {#if phase === 'error'}
      <span class="dot err"></span>
      <span class="label">Dictation failed</span>
    {:else if phase === 'finalizing'}
      <span class="dot busy"></span>
      <span class="label">Finishing…</span>
    {:else if phase === 'starting'}
      <span class="dot busy"></span>
      <span class="label">Listening (warming up the model)…</span>
      <span class="time">{formatElapsed(voiceElapsedSeconds())}</span>
    {:else}
      <span class="dot rec"></span>
      <span class="label">Listening</span>
      <span class="time">{formatElapsed(voiceElapsedSeconds())}</span>
    {/if}
  </div>

  {#if phase === 'error'}
    <p class="err-msg">{voiceErrorMessage()}</p>
    {#if hasText}
      <p class="text"><span class="committed">{voiceCommitted()}</span></p>
    {/if}
  {:else if hasText}
    <p class="text">
      <span class="committed">{voiceCommitted()}</span><span class="tentative">{voiceTentative()}</span>
    </p>
  {:else if phase === 'listening' && !voiceLive()}
    <p class="hint">This model transcribes when you finish — no live preview.</p>
  {:else}
    <p class="hint">Speak — text appears as the engine commits it.</p>
  {/if}

  <div class="foot">
    {#if phase === 'error'}
      <button type="button" class="btn" onclick={() => dismissVoiceError()}>Dismiss</button>
    {:else}
      <span class="keys">esc cancel</span>
      <button
        type="button"
        class="btn"
        onclick={() => cancelVoiceCapture()}
        disabled={phase === 'finalizing'}>Cancel</button
      >
      <button
        type="button"
        class="btn primary"
        onclick={() => stopVoiceCapture()}
        disabled={phase === 'finalizing'}>Done</button
      >
    {/if}
  </div>
</div>

<style>
  .gr-voice {
    position: absolute;
    top: 44px;
    right: 0;
    width: 340px;
    background: var(--surface);
    border: 1px solid var(--line-2);
    border-radius: 12px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    padding: 12px 14px;
    z-index: 40;
    font-family: var(--sans);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--fg2);
  }
  .head .label {
    flex: 1;
    font-weight: 550;
  }
  .head .time {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtle);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot.rec {
    background: var(--coral);
    animation: gr-voice-pulse 1.4s ease-in-out infinite;
  }
  .dot.busy {
    background: var(--subtle);
    animation: gr-voice-pulse 1.4s ease-in-out infinite;
  }
  .dot.err {
    background: var(--coral);
  }
  @keyframes gr-voice-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 0 rgba(224, 122, 95, 0.35);
    }
    50% {
      box-shadow: 0 0 0 5px rgba(224, 122, 95, 0.08);
    }
  }
  .text {
    margin: 10px 0 0;
    max-height: 140px;
    overflow-y: auto;
    font-size: 13px;
    line-height: 1.5;
    color: var(--fg);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .text .tentative {
    color: var(--subtle);
    font-style: italic;
  }
  .hint,
  .err-msg {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--subtle);
  }
  .err-msg {
    color: var(--coral);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .foot .keys {
    flex: 1;
    font-size: 10.5px;
    color: var(--faint);
    font-family: var(--mono);
  }
  .btn {
    height: 26px;
    padding: 0 12px;
    border-radius: 7px;
    border: 1px solid var(--line-2);
    background: transparent;
    color: var(--fg2);
    font-size: 12px;
    font-family: var(--sans);
    cursor: pointer;
    transition: all 0.14s;
  }
  .btn:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
  }
  .btn.primary {
    background: var(--coral);
    border-color: var(--coral);
    color: #fff;
  }
  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
