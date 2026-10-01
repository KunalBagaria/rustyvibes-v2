import { decodeRvw, type RvwHeader } from "../../shared/rvw";

export interface LoadedPack {
  id: string;
  header: RvwHeader;
  buffer: AudioBuffer;
}

const cache = new Map<string, Promise<LoadedPack>>();

/**
 * Fetches and decodes a web pack into one AudioBuffer. Needs no AudioContext, so it can
 * run before the visitor's first gesture; failed loads can be retried.
 */
export function loadPack(id: string, url: string): Promise<LoadedPack> {
  let pending = cache.get(id);
  if (!pending) {
    pending = (async () => {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`couldn't load ${id} (${response.status})`);
      const { header, pcm } = decodeRvw(await response.arrayBuffer());
      const samples = new Float32Array(pcm.length);
      for (let i = 0; i < pcm.length; i++) samples[i] = pcm[i]! / 32768;
      const buffer = new AudioBuffer({ numberOfChannels: 1, length: pcm.length, sampleRate: header.sampleRate });
      buffer.copyToChannel(samples, 0);
      return { id, header, buffer };
    })();
    pending.catch(() => cache.delete(id));
    cache.set(id, pending);
  }
  return pending;
}

/** Warms the HTTP cache for a pack the visitor is about to choose. */
export function prefetchPack(url: string): void {
  void fetch(url, { priority: "low" } as RequestInit).catch(() => {});
}
