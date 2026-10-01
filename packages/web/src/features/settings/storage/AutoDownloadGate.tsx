/**
 * TG-509: wraps a photo or video body. When the viewer's automatic-download rule for the current
 * network allows it, the media loads as before; otherwise a «点击下载 · 2.3 MB» placeholder stands
 * in until tapped (Telegram's behaviour). The rule is read live from the settings store.
 */
import { useState, type ReactNode } from 'react'
import type { AutoDownloadKind } from '@tg/core'
import { networkTypeOf, settingsStore, shouldAutoDownload } from '@tg/core'
import { useStore } from 'zustand/react'
import { t } from '../../../i18n/index'

function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${Math.max(1, Math.round(bytes / 1024))} KB`
}

function currentNetwork() {
  const connection = (globalThis.navigator as { connection?: { type?: string; saveData?: boolean } } | undefined)
    ?.connection
  return networkTypeOf(connection)
}

export function AutoDownloadGate({
  kind,
  sizeBytes,
  children,
}: {
  kind: AutoDownloadKind
  sizeBytes: number
  children: ReactNode
}) {
  const rules = useStore(settingsStore, (state) => state.autoDownload)
  const [tapped, setTapped] = useState(false)
  if (tapped || shouldAutoDownload(rules, currentNetwork(), kind, sizeBytes)) return <>{children}</>
  return (
    <button
      type="button"
      className="tg-autodownload"
      onClick={(event) => {
        event.stopPropagation()
        setTapped(true)
      }}
    >
      <span className="tg-autodownload__icon" aria-hidden="true">
        ↓
      </span>
      <span>
        {kind === 'video' ? t('w.settings.fa4e33') : t('w.settings.be8da6')} · {formatSize(sizeBytes)}
      </span>
    </button>
  )
}
