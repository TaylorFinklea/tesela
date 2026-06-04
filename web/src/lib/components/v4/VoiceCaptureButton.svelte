<script lang="ts">
  /*
   * Voice capture button for the v4 top bar. Records mic audio via
   * MediaRecorder, uploads to /api/transcription/transcribe, and
   * prepends the transcript as a block on today's daily.
   *
   * The recording state is local. While recording, the button shows
   * a red dot and the elapsed seconds; tapping again stops + uploads.
   */
  import { onDestroy } from "svelte";
  import { api } from "$lib/api-client";
  import { apiBase } from "$lib/runtime-base";

  let recording = $state(false);
  let transcribing = $state(false);
  let elapsed = $state(0);
  let error = $state<string | null>(null);
  let lastTranscript = $state<string | null>(null);

  let mediaRecorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];
  let stream: MediaStream | null = null;
  let timer: ReturnType<typeof setInterval> | null = null;

  async function start() {
    error = null;
    lastTranscript = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      // The browser picks the format — Safari → mp4/aac, Chrome → webm/opus.
      // The server decodes WAV only; let the browser handle conversion to WAV
      // by selecting `audio/wav` when supported, otherwise we'll convert via
      // an AudioContext after stop.
      const mime = MediaRecorder.isTypeSupported("audio/wav")
        ? "audio/wav"
        : MediaRecorder.isTypeSupported("audio/webm;codecs=opus")
          ? "audio/webm;codecs=opus"
          : "";
      mediaRecorder = new MediaRecorder(stream, mime ? { mimeType: mime } : undefined);
      chunks = [];
      mediaRecorder.addEventListener("dataavailable", (e) => {
        if (e.data && e.data.size > 0) chunks.push(e.data);
      });
      mediaRecorder.addEventListener("stop", onStop);
      mediaRecorder.start();
      recording = true;
      elapsed = 0;
      timer = setInterval(() => (elapsed += 1), 1000);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function stop() {
    if (!mediaRecorder || mediaRecorder.state === "inactive") return;
    mediaRecorder.stop();
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
    recording = false;
  }

  async function onStop() {
    try {
      const sourceBlob = new Blob(chunks, {
        type: mediaRecorder?.mimeType || "audio/webm",
      });
      const wavBlob =
        sourceBlob.type.startsWith("audio/wav")
          ? sourceBlob
          : await convertToWav(sourceBlob);
      transcribing = true;
      const text = await uploadAndTranscribe(wavBlob);
      transcribing = false;
      lastTranscript = text.trim();
      if (lastTranscript) {
        await appendToToday(lastTranscript);
      } else {
        error = "No speech recognized.";
      }
    } catch (e: unknown) {
      transcribing = false;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      stream?.getTracks().forEach((t) => t.stop());
      stream = null;
      mediaRecorder = null;
      chunks = [];
    }
  }

  async function uploadAndTranscribe(audio: Blob): Promise<string> {
    const form = new FormData();
    form.append("audio", audio, "recording.wav");
    const r = await fetch(`${apiBase()}/transcription/transcribe`, {
      method: "POST",
      body: form,
    });
    if (!r.ok) {
      const t = await r.text();
      throw new Error(`HTTP ${r.status}: ${t}`);
    }
    const data = await r.json();
    return data.text as string;
  }

  /** Decode the recorded blob, downmix to mono 16kHz, write a WAV.
   *  Safari skips this branch since it gives us WAV directly. */
  async function convertToWav(blob: Blob): Promise<Blob> {
    const arrayBuffer = await blob.arrayBuffer();
    const audioCtx = new AudioContext({ sampleRate: 16000 });
    const decoded = await audioCtx.decodeAudioData(arrayBuffer);
    // Mix down to mono.
    const numCh = decoded.numberOfChannels;
    const length = decoded.length;
    const mono = new Float32Array(length);
    for (let ch = 0; ch < numCh; ch++) {
      const data = decoded.getChannelData(ch);
      for (let i = 0; i < length; i++) mono[i] += data[i] / numCh;
    }
    return encodeWav(mono, decoded.sampleRate);
  }

  function encodeWav(samples: Float32Array, sampleRate: number): Blob {
    const buf = new ArrayBuffer(44 + samples.length * 2);
    const view = new DataView(buf);
    // RIFF header
    writeStr(view, 0, "RIFF");
    view.setUint32(4, 36 + samples.length * 2, true);
    writeStr(view, 8, "WAVE");
    writeStr(view, 12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);          // PCM
    view.setUint16(22, 1, true);          // mono
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, sampleRate * 2, true); // byte rate
    view.setUint16(32, 2, true);          // block align
    view.setUint16(34, 16, true);         // bits per sample
    writeStr(view, 36, "data");
    view.setUint32(40, samples.length * 2, true);
    let offset = 44;
    for (let i = 0; i < samples.length; i++, offset += 2) {
      const s = Math.max(-1, Math.min(1, samples[i]));
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
    }
    return new Blob([view], { type: "audio/wav" });
  }

  function writeStr(view: DataView, offset: number, str: string) {
    for (let i = 0; i < str.length; i++) {
      view.setUint8(offset + i, str.charCodeAt(i));
    }
  }

  async function appendToToday(text: string) {
    const today = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
    try {
      const note = await api.getNote(today);
      const blockLine = `- ${text}`;
      const newContent = `${note.content.trimEnd()}\n${blockLine}\n`;
      await api.updateNote(today, newContent);
    } catch (e) {
      // Daily might not exist yet — let the server's lazy-create
      // handle this via /notes/daily.
      const daily = await api.getDailyNote();
      const blockLine = `- ${text}`;
      const newContent = `${daily.content.trimEnd()}\n${blockLine}\n`;
      await api.updateNote(daily.id, { content: newContent });
    }
  }

  onDestroy(() => {
    if (mediaRecorder?.state === "recording") mediaRecorder.stop();
    if (timer) clearInterval(timer);
    stream?.getTracks().forEach((t) => t.stop());
  });

  function elapsedLabel(): string {
    const m = Math.floor(elapsed / 60);
    const s = elapsed % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
</script>

<button
  type="button"
  class="voice-btn"
  class:recording
  class:transcribing
  onclick={() => (recording ? stop() : start())}
  disabled={transcribing}
  title={recording ? "Stop · uploads + transcribes" : "Voice capture — record + transcribe to today"}
>
  {#if transcribing}
    <span class="dot transcribing"></span>
    <span class="label mono">…</span>
  {:else if recording}
    <span class="dot rec"></span>
    <span class="label mono">{elapsedLabel()}</span>
  {:else}
    🎙
  {/if}
</button>

{#if error}
  <span class="err mono" title={error}>!</span>
{/if}

<style>
  .voice-btn {
    background: transparent;
    border: 0;
    color: var(--fg-muted);
    cursor: pointer;
    padding: 4px 8px;
    font-family: inherit;
    font-size: 13px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 4px;
  }
  .voice-btn:hover {
    color: var(--fg-default);
    background: var(--bg-2);
  }
  .voice-btn.recording {
    color: var(--type-task);
  }
  .voice-btn.transcribing {
    color: var(--fg-faint);
    cursor: wait;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, "JetBrains Mono", monospace);
    font-size: 11px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.rec {
    background: var(--type-task);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dot.transcribing {
    background: var(--fg-faint);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
  .err {
    color: var(--type-task);
    margin-left: 4px;
    cursor: help;
  }
</style>
