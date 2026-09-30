import { describe, expect, test } from 'bun:test'
import { TgsError } from '@tg/core/domain'
import { createWorkerEngine, workerPoolSize, type WorkerLike } from '../workerEngine'
import type { FromWorker, ToWorker } from '../worker/protocol'
import { header } from './managerHarness'

type Listener = (event: MessageEvent<FromWorker>) => void

/** A worker double that answers the protocol synchronously-ish and records traffic. */
function fakeWorker(log: Array<{ worker: number; message: ToWorker }>, index: number, boots = true): WorkerLike {
  const listeners: Listener[] = []
  const errors: Array<(event: Event) => void> = []
  const emit = (data: FromWorker) =>
    queueMicrotask(() => listeners.forEach((listener) => listener({ data } as MessageEvent<FromWorker>)))
  setTimeout(() => (boots ? emit({ op: 'ready' }) : errors.forEach((listener) => listener(new Event('error')))), 0)
  return {
    postMessage(message) {
      log.push({ worker: index, message })
      switch (message.op) {
        case 'load':
          if (message.bytes[0] === 0) emit({ op: 'reply', id: message.id, ok: false, code: 'not-gzip', message: 'x' })
          else emit({ op: 'reply', id: message.id, ok: true, value: header() })
          return
        case 'create':
          emit({ op: 'reply', id: message.id, ok: true, value: { frames: 60, frameRate: 60 } })
          return
        case 'render':
          emit({ op: 'frame', unit: message.unit, frame: message.frame, bitmap: { close() {} } as ImageBitmap })
          return
        case 'snapshot':
          emit({ op: 'reply', id: message.id, ok: true, value: new Blob(['png']) })
      }
    },
    addEventListener(type: 'message' | 'error', listener: Listener | ((event: Event) => void)) {
      if (type === 'message') listeners.push(listener as Listener)
      else errors.push(listener as (event: Event) => void)
    },
    terminate() {},
  }
}

describe('workerEngine', () => {
  test('pool size: about half the cores, between 1 and 4', () => {
    expect([2, 4, 8, 16, 64].map(workerPoolSize)).toEqual([1, 1, 3, 4, 4])
  })

  test('each sticker is pinned to the least-loaded worker and parsed there once', async () => {
    const log: Array<{ worker: number; message: ToWorker }> = []
    let index = 0
    const engine = await createWorkerEngine(() => fakeWorker(log, index++), 2)
    await engine.load('a', new Uint8Array([1]))
    await engine.load('b', new Uint8Array([1]))
    const unit = await engine.createUnit('a', 100)
    const loads = log.filter((entry) => entry.message.op === 'load').map((entry) => entry.worker)
    expect(loads).toEqual([0, 1])
    expect(log.find((entry) => entry.message.op === 'create')?.worker).toBe(0)
    expect(unit).toMatchObject({ frames: 60, frameRate: 60 })
  })

  test('one frame in flight per unit; the delivery clears it', async () => {
    const log: Array<{ worker: number; message: ToWorker }> = []
    const engine = await createWorkerEngine(() => fakeWorker(log, 0), 1)
    await engine.load('a', new Uint8Array([1]))
    const unit = await engine.createUnit('a', 100)
    const delivered: unknown[] = []
    expect(unit.request(3, (image) => delivered.push(image))).toBe(true)
    expect(unit.request(4, (image) => delivered.push(image))).toBe(false)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(delivered).toHaveLength(1)
    expect(unit.request(4, (image) => delivered.push(image))).toBe(true)
  })

  test('a bad file rejects with the worker-side TgsError code', async () => {
    const engine = await createWorkerEngine(() => fakeWorker([], 0), 1)
    const failure = await engine.load('bad', new Uint8Array([0])).catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(TgsError)
    expect((failure as TgsError).code).toBe('not-gzip')
  })

  test('a worker that fails to boot rejects the engine (the loader then falls back)', async () => {
    await expect(createWorkerEngine(() => fakeWorker([], 0, false), 1)).rejects.toThrow('render worker failed')
  })
})
