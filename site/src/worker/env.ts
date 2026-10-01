/** The subset of an R2 object the Worker uses (structurally compatible with R2Object/R2ObjectBody). */
export interface StoredObject {
  size: number;
  httpEtag: string;
  range?: { offset?: number; length?: number; suffix?: number };
  body?: ReadableStream;
  writeHttpMetadata(headers: Headers): void;
}

export interface ReleaseStore {
  get(key: string, options?: { range?: Headers; onlyIf?: Headers }): Promise<StoredObject | null>;
  head(key: string): Promise<StoredObject | null>;
}

export interface AssetFetcher {
  fetch(request: Request): Promise<Response>;
}

export interface Env {
  ASSETS: AssetFetcher;
  RELEASES: ReleaseStore;
}
