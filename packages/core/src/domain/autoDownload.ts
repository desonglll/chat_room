/**
 * TG-509 automatic media download (Telegram: Data and Storage › Automatic media download):
 * per network type, which media kinds load without a tap, with a size cap for videos and files.
 * Pure — the web layer supplies the network type and the attachment.
 */

export type NetworkType = 'wifi' | 'cellular' | 'roaming'
export type AutoDownloadKind = 'photo' | 'video' | 'file'

export interface AutoDownloadRule {
  photo: boolean
  video: boolean
  file: boolean
  /** Videos and files larger than this are never downloaded automatically. */
  maxBytes: number
}

export type AutoDownloadRules = Record<NetworkType, AutoDownloadRule>

const MB = 1024 * 1024

/** Telegram's defaults: everything small on Wi-Fi, photos on mobile data, nothing roaming. */
export const DEFAULT_AUTO_DOWNLOAD: AutoDownloadRules = {
  wifi: { photo: true, video: true, file: true, maxBytes: 15 * MB },
  cellular: { photo: true, video: false, file: false, maxBytes: 1 * MB },
  roaming: { photo: false, video: false, file: false, maxBytes: 0 },
}

/** Photos are always small enough; videos and files respect the cap. */
export function shouldAutoDownload(
  rules: AutoDownloadRules,
  network: NetworkType,
  kind: AutoDownloadKind,
  sizeBytes: number,
): boolean {
  const rule = rules[network]
  if (!rule[kind]) return false
  return kind === 'photo' || sizeBytes <= rule.maxBytes
}

/** Map the Network Information API (where present) to a rule set; unknown counts as Wi-Fi. */
export function networkTypeOf(connection: { type?: string; saveData?: boolean } | undefined): NetworkType {
  if (!connection) return 'wifi'
  if (connection.saveData) return 'roaming'
  return connection.type === 'cellular' ? 'cellular' : 'wifi'
}
