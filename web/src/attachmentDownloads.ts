import type { Attachment } from './types'

const MAX_FILES_PER_BATCH = 20
const MAX_BYTES_PER_BATCH = 200 * 1024 * 1024

export interface DownloadProgress {
  stage: 'downloading' | 'packing'
  completedFiles: number
  totalFiles: number
  receivedBytes: number
  totalBytes: number
  batchIndex: number
  batchCount: number
  percent: number
}

interface DownloadOptions {
  signal: AbortSignal
  onProgress: (progress: DownloadProgress) => void
}

export function partitionAttachmentBatches(
  attachments: Attachment[],
  maxFiles = MAX_FILES_PER_BATCH,
  maxBytes = MAX_BYTES_PER_BATCH,
): Attachment[][] {
  const batches: Attachment[][] = []
  let batch: Attachment[] = []
  let bytes = 0
  for (const attachment of attachments) {
    const overLimit = batch.length > 0 && (batch.length >= maxFiles || bytes + attachment.size_bytes > maxBytes)
    if (overLimit) {
      batches.push(batch)
      batch = []
      bytes = 0
    }
    batch.push(attachment)
    bytes += attachment.size_bytes
  }
  if (batch.length) batches.push(batch)
  return batches
}

export async function downloadAttachmentArchives(
  attachments: Attachment[],
  archiveName: string,
  options: DownloadOptions,
): Promise<void> {
  if (!attachments.length) return
  const { default: JSZip } = await import('jszip')
  const batches = partitionAttachmentBatches(attachments)
  const totalBytes = attachments.reduce((sum, attachment) => sum + attachment.size_bytes, 0)
  let receivedBytes = 0
  let completedFiles = 0
  let packedBatches = 0

  for (const [batchOffset, batch] of batches.entries()) {
    assertNotAborted(options.signal)
    const zip = new JSZip()
    const used = new Set<string>()
    for (const [fileOffset, attachment] of batch.entries()) {
      const response = await fetch(attachment.download_url, { signal: options.signal })
      if (!response.ok) throw new Error(`下载 ${attachment.file_name} 失败`)
      const blob = await readBlob(response, options.signal, (bytes) => {
        receivedBytes += bytes
        options.onProgress(
          progress(
            'downloading',
            completedFiles,
            attachments.length,
            receivedBytes,
            totalBytes,
            batchOffset,
            batches.length,
            packedBatches,
            0,
          ),
        )
      })
      zip.file(uniqueName(attachment.file_name, fileOffset, used), blob)
      completedFiles += 1
    }
    const blob = await zip.generateAsync({ type: 'blob', compression: 'STORE' }, (metadata) => {
      assertNotAborted(options.signal)
      options.onProgress(
        progress(
          'packing',
          completedFiles,
          attachments.length,
          receivedBytes,
          totalBytes,
          batchOffset,
          batches.length,
          packedBatches,
          metadata.percent,
        ),
      )
    })
    assertNotAborted(options.signal)
    packedBatches += 1
    saveBlob(blob, archiveFileName(archiveName, batchOffset, batches.length))
  }
}

async function readBlob(response: Response, signal: AbortSignal, onChunk: (bytes: number) => void): Promise<Blob> {
  if (!response.body) {
    const blob = await response.blob()
    onChunk(blob.size)
    return blob
  }
  const reader = response.body.getReader()
  const chunks: Uint8Array[] = []
  try {
    while (true) {
      assertNotAborted(signal)
      const { done, value } = await reader.read()
      if (done) break
      chunks.push(value)
      onChunk(value.byteLength)
    }
  } finally {
    reader.releaseLock()
  }
  return new Blob(chunks as BlobPart[])
}

function progress(
  stage: DownloadProgress['stage'],
  completedFiles: number,
  totalFiles: number,
  receivedBytes: number,
  totalBytes: number,
  batchOffset: number,
  batchCount: number,
  packedBatches: number,
  packingPercent: number,
): DownloadProgress {
  const downloadRatio = totalBytes > 0 ? Math.min(receivedBytes / totalBytes, 1) : completedFiles / totalFiles
  const packRatio = (packedBatches + (stage === 'packing' ? packingPercent / 100 : 0)) / batchCount
  return {
    stage,
    completedFiles,
    totalFiles,
    receivedBytes,
    totalBytes,
    batchIndex: batchOffset + 1,
    batchCount,
    percent: Math.min(100, Math.round(downloadRatio * 85 + packRatio * 15)),
  }
}

function uniqueName(name: string, index: number, used: Set<string>): string {
  if (!used.has(name)) {
    used.add(name)
    return name
  }
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  const extension = dot > 0 ? name.slice(dot) : ''
  let suffix = index + 1
  let candidate = `${stem}-${suffix}${extension}`
  while (used.has(candidate)) candidate = `${stem}-${++suffix}${extension}`
  used.add(candidate)
  return candidate
}

function archiveFileName(name: string, batchOffset: number, batchCount: number): string {
  const base = name.trim() || 'chat-files'
  return batchCount === 1 ? `${base}.zip` : `${base}-${batchOffset + 1}-of-${batchCount}.zip`
}

function saveBlob(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = name
  link.click()
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}

function assertNotAborted(signal: AbortSignal): void {
  if (signal.aborted) throw new DOMException('Download cancelled', 'AbortError')
}
