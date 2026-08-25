import { afterEach, describe, expect, test } from 'bun:test'
import { uploadDirect, type DirectUploadTarget } from './attachmentUploadApi'

const originalXhr = globalThis.XMLHttpRequest

class FakeXMLHttpRequest {
  static responseStatus = 204
  static latest: FakeXMLHttpRequest

  method = ''
  url = ''
  status = 0
  body: XMLHttpRequestBodyInit | null = null
  headers: Record<string, string> = {}
  upload: { onprogress: ((event: ProgressEvent) => void) | null } = { onprogress: null }
  onload: (() => void) | null = null
  onerror: (() => void) | null = null
  onabort: (() => void) | null = null

  constructor() {
    FakeXMLHttpRequest.latest = this
  }

  open(method: string, url: string): void {
    this.method = method
    this.url = url
  }

  setRequestHeader(name: string, value: string): void {
    this.headers[name] = value
  }

  send(body: XMLHttpRequestBodyInit | null): void {
    this.body = body
    const file = body as File
    this.upload.onprogress?.({ lengthComputable: true, loaded: file.size, total: file.size } as ProgressEvent)
    this.status = FakeXMLHttpRequest.responseStatus
    this.onload?.()
  }

  abort(): void {
    this.onabort?.()
  }
}

afterEach(() => {
  globalThis.XMLHttpRequest = originalXhr
  FakeXMLHttpRequest.responseStatus = 204
})

describe('direct OSS upload', () => {
  const target: DirectUploadTarget = {
    method: 'PUT',
    url: 'https://bucket.example/object?signature=short-lived',
    headers: { 'content-type': 'text/plain' },
    expires_at: '2026-08-26T00:00:00Z',
  }

  test('sends the file to the signed URL without application credentials', async () => {
    globalThis.XMLHttpRequest = FakeXMLHttpRequest as unknown as typeof XMLHttpRequest
    const file = new File(['direct bytes'], 'direct.txt', { type: 'text/plain' })
    const progress: number[] = []

    await uploadDirect(target, file, (bytes) => progress.push(bytes))

    expect(FakeXMLHttpRequest.latest.method).toBe('PUT')
    expect(FakeXMLHttpRequest.latest.url).toBe(target.url)
    expect(FakeXMLHttpRequest.latest.headers).toEqual({ 'content-type': 'text/plain' })
    expect(FakeXMLHttpRequest.latest.body).toBe(file)
    expect(progress.at(-1)).toBe(file.size)
  })

  test('rejects an OSS response so the caller can use chunked fallback', async () => {
    globalThis.XMLHttpRequest = FakeXMLHttpRequest as unknown as typeof XMLHttpRequest
    FakeXMLHttpRequest.responseStatus = 403
    const file = new File(['denied'], 'denied.txt', { type: 'text/plain' })

    await expect(uploadDirect(target, file, () => {})).rejects.toThrow('OSS 直传失败：403')
  })
})
