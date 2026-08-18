import type { Attachment } from './types'

function uniqueName(name: string, index: number, used: Set<string>): string {
  if (!used.has(name)) {
    used.add(name)
    return name
  }
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  const extension = dot > 0 ? name.slice(dot) : ''
  const candidate = `${stem}-${index + 1}${extension}`
  used.add(candidate)
  return candidate
}

export async function downloadAttachmentArchive(
  attachments: Attachment[],
  archiveName: string,
): Promise<void> {
  if (!attachments.length) return
  const { default: JSZip } = await import('jszip')
  const zip = new JSZip()
  const used = new Set<string>()
  for (const [index, attachment] of attachments.entries()) {
    const response = await fetch(attachment.download_url)
    if (!response.ok) throw new Error(`下载 ${attachment.file_name} 失败`)
    zip.file(uniqueName(attachment.file_name, index, used), await response.blob())
  }
  const blob = await zip.generateAsync({ type: 'blob', compression: 'DEFLATE' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `${archiveName || 'chat-files'}.zip`
  link.click()
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}
