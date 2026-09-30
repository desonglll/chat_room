/**
 * The injected platform capabilities (architecture.md §2).
 *
 * `packages/core` may not touch DOM globals, so every platform ability is an interface the
 * host constructs and hands in: `createStorage()`, `createWebSocket()`, `createFileReader()`
 * and `createClock()` are implemented by packages/web (browser) today and by the React
 * Native host later. Core only defines the shapes and consumes instances.
 */

/** What `createStorage()` returns: a string key/value store (browser web storage, RN async storage, …). */
export interface CoreStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export type CoreSocketCloseInfo = { code: number; reason: string }

/**
 * The subset of the WHATWG WebSocket surface the realtime client uses. Handler properties
 * (not addEventListener) on purpose: it is the least common denominator across browser,
 * bun and RN sockets, and it keeps fakes trivial.
 */
export interface CoreSocket {
  send(data: string): void
  close(code?: number, reason?: string): void
  onopen: (() => void) | null
  onmessage: ((data: string) => void) | null
  onclose: ((info: CoreSocketCloseInfo) => void) | null
  onerror: (() => void) | null
}

/** What `createWebSocket()` is: a factory the host injects, one socket per call. */
export type CoreSocketFactory = (url: string) => CoreSocket

/** A chunk-readable file source, however the host stores files. */
export interface FileChunkSource {
  size: number
  /** Bytes `[start, end)`, like `Blob.slice(start, end).arrayBuffer()`. */
  readChunk(start: number, end: number): Promise<Uint8Array>
}

/** What `createFileReader()` returns: an adapter from the host's file handle to a chunk source. */
export type CoreFileReader<HostFile> = (file: HostFile) => FileChunkSource

/** Opaque timer handle; whatever the host clock's setTimeout returns. */
export type CoreTimerHandle = unknown

/**
 * What `createClock()` returns. Injected (instead of the timer globals) so reconnect and
 * expiry logic is deterministic under test and portable off-DOM.
 */
export interface CoreClock {
  /** Milliseconds since the Unix epoch. */
  now(): number
  setTimeout(callback: () => void, delayMs: number): CoreTimerHandle
  clearTimeout(handle: CoreTimerHandle): void
}

/** An incremental SHA-256, host-supplied (see docs/devlog/TG-011.md Decisions). */
export interface Sha256Stream {
  update(bytes: Uint8Array): void
  /** Lowercase hex digest; the stream is finished afterwards. */
  digestHex(): string
}

export type Sha256StreamFactory = () => Sha256Stream
