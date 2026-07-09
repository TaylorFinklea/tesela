/**
 * Live voice capture controller + reactive state (dictation P2).
 *
 * Owns the whole session: mic (getUserMedia) → AudioContext → the
 * /voice-worklet.js chunker (which downsamples to 16 kHz) → binary WS
 * frames to /transcription/stream → committed/tentative partials back →
 * on stop, the final transcript is appended to today's daily note.
 *
 * The mic starts the moment the socket opens — audio buffers through
 * the server's bounded channel while the model loads, so the first
 * words aren't lost even when `ready` takes a couple of seconds.
 *
 * Concurrency: a session is a value machine guarded by `generation`. A
 * new session or any terminal transition (cancel/fail/complete) bumps
 * it; every async continuation captures the generation it started under
 * and bails (releasing whatever it just acquired) when it no longer
 * matches. This is what stops a cancel mid-`getUserMedia` from leaking
 * a live mic, and a stop mid-connect from crashing the pipeline.
 */
import { api } from "$lib/api-client";
import { toast } from "$lib/stores/toast.svelte";
import { parseServerFrame, STOP_FRAME } from "./protocol";

export type VoicePhase =
  | "idle"
  /** Socket open + mic live, model still loading server-side. */
  | "starting"
  /** Session ready; partials arrive if the model streams. */
  | "listening"
  /** Stop sent; waiting for the final transcript. */
  | "finalizing"
  | "error";

/** Backstop for a server that neither sends `final` nor closes after
 *  stop. Generous because buffered streaming can take tens of seconds
 *  to drain a long utterance on stop. */
const FINALIZE_WATCHDOG_MS = 180_000;

let phase = $state<VoicePhase>("idle");
/** True when the active model streams partials (`ready.streaming`). */
let live = $state(false);
let committed = $state("");
let tentative = $state("");
let errorMessage = $state("");
let elapsedSeconds = $state(0);

/** Monotonic session token; see the module doc. */
let generation = 0;
let socket: WebSocket | null = null;
let audioCtx: AudioContext | null = null;
let mediaStream: MediaStream | null = null;
let worklet: AudioWorkletNode | null = null;
let timer: ReturnType<typeof setInterval> | null = null;
let watchdog: ReturnType<typeof setTimeout> | null = null;

export function voicePhase(): VoicePhase {
  return phase;
}
export function voiceLive(): boolean {
  return live;
}
export function voiceCommitted(): string {
  return committed;
}
export function voiceTentative(): string {
  return tentative;
}
export function voiceErrorMessage(): string {
  return errorMessage;
}
export function voiceElapsedSeconds(): number {
  return elapsedSeconds;
}
/** Popover visibility: any non-idle state (incl. error, so the user
 *  can read what went wrong / copy an unsaved transcript). */
export function voiceOpen(): boolean {
  return phase !== "idle";
}

/** Mic button + `voice` command entry point. */
export function toggleVoiceCapture(): void {
  if (phase === "idle" || phase === "error") {
    void startVoiceCapture();
  } else if (phase === "starting" || phase === "listening") {
    stopVoiceCapture();
  }
  // finalizing: ignore — the session is already ending.
}

/** Same-origin WS URL — mirrors ws-client.svelte.ts's wsUrl() so it
 *  works identically under the vite dev proxy and the Tauri desktop. */
function voiceWsUrl(): string {
  if (typeof window === "undefined") return "ws://127.0.0.1:7474/transcription/stream";
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/transcription/stream`;
}

export async function startVoiceCapture(): Promise<void> {
  if (phase !== "idle" && phase !== "error") return;
  const gen = ++generation; // invalidates any still-pending prior session
  committed = "";
  tentative = "";
  errorMessage = "";
  live = false;
  elapsedSeconds = 0;
  phase = "starting";

  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });
  } catch {
    if (gen === generation) fail("Microphone unavailable — check the browser/app permission.");
    return;
  }
  // Cancelled while the permission prompt / device was pending: the
  // stream is live NOW, so stop it or it's a hot-mic leak with no UI.
  if (gen !== generation) {
    for (const track of stream.getTracks()) track.stop();
    return;
  }
  mediaStream = stream;

  const ws = new WebSocket(voiceWsUrl());
  socket = ws;
  ws.onmessage = (ev) => {
    if (gen === generation && socket === ws && typeof ev.data === "string") handleFrame(ev.data);
  };
  ws.onerror = () => {
    if (gen === generation && (phase === "starting" || phase === "listening" || phase === "finalizing")) {
      fail("Dictation connection failed.");
    }
  };
  ws.onclose = () => {
    if (gen !== generation) return;
    if (phase === "finalizing") {
      // Clean close without a `final` frame: don't hang on the spinner
      // — save whatever committed text we have, else end quietly.
      void completeWith(committed);
    } else if (phase === "starting" || phase === "listening") {
      fail("Dictation connection closed unexpectedly.");
    }
  };
  ws.onopen = () => {
    if (gen !== generation || socket !== ws) return;
    if (phase === "finalizing") {
      // Stop was pressed while still connecting: skip the audio path
      // and finalize immediately (no audio was ever captured).
      ws.send(STOP_FRAME);
      armWatchdog(gen);
      return;
    }
    void startAudioPipeline(ws, gen);
  };
}

async function startAudioPipeline(ws: WebSocket, gen: number): Promise<void> {
  if (gen !== generation || socket !== ws || !mediaStream) return;
  try {
    // Request a 16 kHz context (the server's PCM rate). Engines that
    // honor it let the worklet pass audio through untouched; those that
    // ignore it (e.g. WKWebView) hand back 44.1/48 kHz and the worklet
    // downsamples to 16 kHz itself, so either way the server gets 16 kHz.
    const ctx = new AudioContext({ sampleRate: 16_000 });
    await ctx.audioWorklet.addModule("/voice-worklet.js");
    // Stop / cancel may have fired during the module load: don't wire a
    // graph onto a session that's already ending (that threw before).
    if (gen !== generation || socket !== ws) {
      void ctx.close();
      return;
    }
    audioCtx = ctx;
    const source = ctx.createMediaStreamSource(mediaStream);
    worklet = new AudioWorkletNode(ctx, "tesela-voice-chunker");
    worklet.port.onmessage = (ev: MessageEvent<ArrayBuffer>) => {
      if (gen === generation && ws.readyState === WebSocket.OPEN) ws.send(ev.data);
    };
    source.connect(worklet);
    startTimer();
  } catch (e) {
    if (gen === generation) fail(`Audio pipeline failed: ${e instanceof Error ? e.message : e}`);
  }
}

function handleFrame(raw: string): void {
  const frame = parseServerFrame(raw);
  if (!frame) return;
  switch (frame.type) {
    case "ready":
      live = frame.streaming;
      if (phase === "starting") phase = "listening";
      break;
    case "partial":
      committed = frame.committed;
      tentative = frame.tentative;
      break;
    case "final":
      void completeWith(frame.text);
      break;
    case "error":
      fail(frame.message);
      break;
  }
}

/** Finish the session: mic off, stop frame out, wait for `final`. */
export function stopVoiceCapture(): void {
  if (phase !== "starting" && phase !== "listening") return;
  phase = "finalizing";
  // Tear the audio path down FIRST so every already-captured chunk is
  // on the wire before the stop control frame (WS ordering does the rest).
  teardownAudio();
  if (!socket) {
    fail("Dictation connection lost before finishing.");
    return;
  }
  if (socket.readyState === WebSocket.OPEN) {
    socket.send(STOP_FRAME);
    armWatchdog(generation);
  } else if (socket.readyState === WebSocket.CONNECTING) {
    // The onopen handler sees phase === "finalizing" and sends stop then.
  } else {
    fail("Dictation connection lost before finishing.");
  }
}

/** Abort without a transcript (Esc / close). No-op during finalizing so
 *  a stray Esc can't throw away a transcript the server is still
 *  sending. */
export function cancelVoiceCapture(): void {
  if (phase === "finalizing") return;
  ++generation; // invalidate any pending continuation
  teardownAll();
  phase = "idle";
}

/** Dismiss the error panel. */
export function dismissVoiceError(): void {
  if (phase === "error") phase = "idle";
}

/** Terminal success path: append `text` to today's daily (if any). */
async function completeWith(text: string): Promise<void> {
  const gen = ++generation; // this session is done; ignore late frames
  teardownAll();
  const trimmed = text.trim();
  if (!trimmed) {
    phase = "idle";
    toast("Nothing heard — nothing saved.", "info");
    return;
  }
  try {
    const note = await api.getDailyNote();
    await api.upsertBlocks(note.id, [
      {
        kind: "upsert",
        bid: crypto.randomUUID(),
        text: trimmed,
        parent_bid: null,
        indent_level: 0,
        // no after_bid → append at document end
      },
    ]);
    if (gen === generation) {
      phase = "idle";
      toast("Dictation added to today's daily.", "success");
    }
  } catch {
    if (gen === generation) {
      // Keep the transcript on screen so nothing is lost.
      committed = trimmed;
      tentative = "";
      errorMessage = "Couldn't save to today's daily — transcript kept above.";
      phase = "error";
    }
  }
}

function fail(message: string): void {
  ++generation;
  teardownAll();
  errorMessage = message;
  phase = "error";
}

/** Watchdog: if neither `final` nor a close arrives after stop, save
 *  what we have rather than spinning forever on "Finishing…". */
function armWatchdog(gen: number): void {
  clearWatchdog();
  watchdog = setTimeout(() => {
    if (gen === generation && phase === "finalizing") {
      void completeWith(committed);
    }
  }, FINALIZE_WATCHDOG_MS);
}

function clearWatchdog(): void {
  if (watchdog) {
    clearTimeout(watchdog);
    watchdog = null;
  }
}

/** Full teardown incl. socket — used by every terminal transition. */
function teardownAll(): void {
  teardownAudio();
  clearWatchdog();
  if (socket) {
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    socket.close();
    socket = null;
  }
}

function teardownAudio(): void {
  stopTimer();
  if (worklet) {
    worklet.port.onmessage = null;
    worklet.disconnect();
    worklet = null;
  }
  if (audioCtx) {
    void audioCtx.close();
    audioCtx = null;
  }
  if (mediaStream) {
    for (const track of mediaStream.getTracks()) track.stop();
    mediaStream = null;
  }
}

function startTimer(): void {
  stopTimer();
  const started = Date.now();
  timer = setInterval(() => {
    elapsedSeconds = (Date.now() - started) / 1000;
  }, 500);
}

function stopTimer(): void {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}
