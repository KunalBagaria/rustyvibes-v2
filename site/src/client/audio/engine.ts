import { DEFAULT_PACK, PACKS } from "../catalog.gen";
import { DEFAULT_VOLUME, limiterCurve, MAX_VOICES, volumeToGain } from "../../shared/sound-math";
import { loadPack, type LoadedPack } from "./packs";
import { pickClip, voiceFor } from "./voice";

export interface PackState {
  pack: string;
  loading: boolean;
  error: string | null;
}

export interface VoiceEvent {
  kvk: number;
  kind: "press" | "release";
  rate: number;
  gain: number;
  pan: number;
}

const LIMITER_RANGE = 4;

/**
 * The page's sound engine, built like the app's: per-voice variation and panning,
 * a volume² master gain and the same soft-knee limiter.
 */
export class AudioEngine {
  enabled = true;
  variation = true;
  spatial = true;
  releaseSounds = true;
  #volume = DEFAULT_VOLUME;
  #ctx: AudioContext | null = null;
  #master: GainNode | null = null;
  #pack: LoadedPack | null = null;
  #packId = DEFAULT_PACK;
  #loading = false;
  #error: string | null = null;
  #voices: AudioBufferSourceNode[] = [];
  #loadPromise: Promise<void> = Promise.resolve();
  #packListeners = new Set<(state: PackState) => void>();
  #voiceListeners = new Set<(voice: VoiceEvent) => void>();

  get packId(): string {
    return this.#packId;
  }

  get state(): PackState {
    return { pack: this.#packId, loading: this.#loading, error: this.#error };
  }

  get volume(): number {
    return this.#volume;
  }

  set volume(value: number) {
    this.#volume = Math.min(1, Math.max(0, value));
    if (this.#ctx && this.#master) {
      this.#master.gain.setTargetAtTime(volumeToGain(this.#volume), this.#ctx.currentTime, 0.015);
    }
  }

  /** Called with the current pack whenever it or its loading state changes. */
  on(listener: (state: PackState) => void): () => void {
    this.#packListeners.add(listener);
    return () => this.#packListeners.delete(listener);
  }

  /** Called for every sound that plays. */
  onVoice(listener: (voice: VoiceEvent) => void): () => void {
    this.#voiceListeners.add(listener);
    return () => this.#voiceListeners.delete(listener);
  }

  #emit(): void {
    const state = this.state;
    for (const listener of this.#packListeners) listener(state);
  }

  #context(): AudioContext {
    if (this.#ctx) return this.#ctx;
    const ctx = new AudioContext({ latencyHint: "interactive" });
    const master = ctx.createGain();
    master.gain.value = volumeToGain(this.#volume);
    const pre = ctx.createGain();
    pre.gain.value = 1 / LIMITER_RANGE;
    const shaper = ctx.createWaveShaper();
    shaper.curve = limiterCurve(4097, LIMITER_RANGE) as Float32Array<ArrayBuffer>;
    master.connect(pre).connect(shaper).connect(ctx.destination);
    this.#ctx = ctx;
    this.#master = master;
    return ctx;
  }

  /** Creates or resumes audio; call from a user gesture. */
  unlock(): void {
    const ctx = this.#context();
    if (ctx.state !== "running") void ctx.resume().catch(() => {});
    if (!this.#pack && !this.#loading) void this.setPack(this.#packId);
  }

  /** Loads a pack without starting audio, so the first key press sounds at once. */
  preload(): void {
    if (this.#pack || this.#loading) return;
    void this.setPack(this.#packId);
  }

  /** Resolves once the current pack is loaded (or has failed to load). */
  ready(): Promise<void> {
    this.preload();
    return this.#loadPromise;
  }

  setPack(id: string): Promise<void> {
    const info = PACKS.find((p) => p.id === id);
    if (!info) return this.#loadPromise;
    this.#packId = id;
    this.#loading = true;
    this.#error = null;
    this.#emit();
    this.#loadPromise = loadPack(id, info.url).then(
      (pack) => {
        if (this.#packId !== id) return;
        this.#pack = pack;
        this.#loading = false;
        this.#emit();
      },
      (error: unknown) => {
        if (this.#packId !== id) return;
        this.#loading = false;
        this.#error = (error as Error).message;
        this.#emit();
      },
    );
    return this.#loadPromise;
  }

  press(kvk: number | undefined): void {
    this.#play(kvk, "press");
  }

  release(kvk: number | undefined): void {
    if (this.releaseSounds) this.#play(kvk, "release");
  }

  #play(kvk: number | undefined, kind: "press" | "release"): void {
    const ctx = this.#ctx;
    const pack = this.#pack;
    const master = this.#master;
    if (!this.enabled || !ctx || !master || !pack || kvk === undefined || pack.id !== this.#packId) return;
    const clip = pickClip(pack.header[kind][String(kvk)], Math.random);
    if (clip === null) return;
    const [start, length] = pack.header.clips[clip]!;
    const voice = voiceFor(kvk, { variation: this.variation, spatial: this.spatial, random: Math.random });
    const source = ctx.createBufferSource();
    source.buffer = pack.buffer;
    source.playbackRate.value = voice.rate;
    const level = ctx.createGain();
    level.gain.value = voice.level;
    const panner = ctx.createStereoPanner();
    panner.pan.value = voice.pan;
    source.connect(level).connect(panner).connect(master);
    const rate = pack.header.sampleRate;
    source.start(0, start / rate, length / rate);
    source.onended = () => {
      this.#voices = this.#voices.filter((v) => v !== source);
      panner.disconnect();
    };
    this.#voices.push(source);
    if (this.#voices.length > MAX_VOICES) this.#voices.shift()?.stop();
    for (const listener of this.#voiceListeners) listener({ kvk, kind, ...voice });
  }
}
