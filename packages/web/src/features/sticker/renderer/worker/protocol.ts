/** Messages between `workerEngine` (page) and `renderWorker` (worker). */
import type { LottieHeader, TgsErrorCode } from '@tg/core/domain'

export type ToWorker =
  | { op: 'load'; id: number; key: string; bytes: Uint8Array }
  | { op: 'unload'; key: string }
  | { op: 'create'; id: number; unit: number; key: string; pixelSize: number }
  | { op: 'render'; unit: number; frame: number }
  | { op: 'destroy'; unit: number }
  | { op: 'snapshot'; id: number; key: string; pixelSize: number }

export interface UnitInfo {
  frames: number
  frameRate: number
}

export type ReplyValue = LottieHeader | UnitInfo | Blob

export type FromWorker =
  | { op: 'ready' }
  | { op: 'reply'; id: number; ok: true; value: ReplyValue }
  | { op: 'reply'; id: number; ok: false; code: TgsErrorCode | null; message: string }
  | { op: 'frame'; unit: number; frame: number; bitmap: ImageBitmap }
