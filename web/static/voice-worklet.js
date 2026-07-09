/*
 * tesela-voice-chunker — AudioWorkletProcessor for live dictation.
 *
 * Runs on the audio thread. Downsamples the input to 16 kHz (the
 * server's PCM contract) and emits 100 ms chunks (1600 samples at
 * 16 kHz), transferring each chunk's ArrayBuffer to the main thread,
 * which forwards it as one binary WebSocket frame.
 *
 * Why resample here instead of trusting the AudioContext rate: we
 * REQUEST a 16 kHz context, but some engines (notably WKWebView, the
 * Tauri desktop's) ignore the request and hand back 44.1/48 kHz. Left
 * unhandled, that PCM would be read by the server as 16 kHz — i.e. ~3x
 * too fast — and transcribe to garbage. `sampleRate` here is the audio
 * thread's ACTUAL rate, so linear-interpolating from it to 16 kHz is
 * correct whether or not the request was honored (ratio 1 = no-op).
 */
const TARGET_RATE = 16000;
const CHUNK = 1600; // 100 ms at 16 kHz

class TeselaVoiceChunker extends AudioWorkletProcessor {
  constructor() {
    super();
    // `sampleRate` is a global in AudioWorkletGlobalScope: the real rate.
    this._ratio = sampleRate / TARGET_RATE;
    this._buf = new Float32Array(CHUNK);
    this._len = 0;
    // Fractional read position into a virtual concatenation of inputs,
    // carried across process() calls so resampling has no seams.
    this._pos = 0;
    this._prevTail = 0; // last sample of the previous quantum (for interp)
  }

  _pushSample(s) {
    this._buf[this._len++] = s;
    if (this._len === CHUNK) {
      const out = this._buf.slice(0);
      this.port.postMessage(out.buffer, [out.buffer]);
      this._len = 0;
    }
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel || channel.length === 0) return true;

    if (this._ratio === 1) {
      for (let i = 0; i < channel.length; i++) this._pushSample(channel[i]);
      return true;
    }

    // Linear-interpolate from the source rate to 16 kHz. `this._pos` is
    // the fractional source index; sample index -1 maps to the tail of
    // the previous quantum so consecutive quanta join seamlessly.
    const n = channel.length;
    while (this._pos < n) {
      const i = Math.floor(this._pos);
      const frac = this._pos - i;
      const a = i < 0 ? this._prevTail : channel[i];
      const b = channel[i + 1 >= n ? n - 1 : i + 1];
      this._pushSample(a + (b - a) * frac);
      this._pos += this._ratio;
    }
    this._pos -= n; // carry fractional remainder into the next quantum
    this._prevTail = channel[n - 1];
    return true;
  }
}

registerProcessor('tesela-voice-chunker', TeselaVoiceChunker);
