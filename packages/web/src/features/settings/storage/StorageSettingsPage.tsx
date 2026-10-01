/**
 * TG-509 «数据与存储»: automatic media download per network type, how much this browser stores
 * (`navigator.storage.estimate`, Cache Storage) with a clear button, and the offline cache's
 * ceiling and retention (applied by the service worker, TG-601). Clearing the cache never
 * touches media already on screen — those are live elements, not cache entries.
 */
import { useCallback, useEffect, useState } from 'react'
import type { AutoDownloadRule, NetworkType } from '@tg/core'
import { settingsStore } from '@tg/core'
import { Button, Toggle } from '@tg/ui'
import { useStore } from 'zustand/react'
import { browserStorage } from '../../../app/platform'

const NETWORKS: { id: NetworkType; label: string }[] = [
  { id: 'wifi', label: 'Wi-Fi' },
  { id: 'cellular', label: '移动数据' },
  { id: 'roaming', label: '漫游 / 省流量' },
]
const SIZE_CAPS = [0, 1, 5, 15, 50, 100].map((mb) => ({
  bytes: mb * 1024 * 1024,
  label: mb === 0 ? '不下载' : `${mb} MB`,
}))
const LIMITS = [256, 512, 1024, 2048, 5120]
const RETENTION = [1, 3, 7, 30, 0]

function mb(bytes: number | undefined): string {
  return bytes === undefined ? '—' : `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

interface Usage {
  used: number | undefined
  quota: number | undefined
  caches: number
}

async function cacheUsage(): Promise<Usage> {
  const estimate = await globalThis.navigator?.storage?.estimate?.().catch(() => undefined)
  const names = (await globalThis.caches?.keys().catch(() => [] as string[])) ?? []
  return { used: estimate?.usage, quota: estimate?.quota, caches: names.length }
}

export function StorageSettingsPage() {
  const settings = useStore(settingsStore)
  const [usage, setUsage] = useState<Usage | null>(null)
  const [cleared, setCleared] = useState(false)
  const refresh = useCallback(() => void cacheUsage().then(setUsage), [])
  useEffect(refresh, [refresh])

  const save = (change: Partial<typeof settings>) => {
    settingsStore.getState().update(change)
    settingsStore.getState().persist(browserStorage)
  }
  const setRule = (network: NetworkType, change: Partial<AutoDownloadRule>) =>
    save({ autoDownload: { ...settings.autoDownload, [network]: { ...settings.autoDownload[network], ...change } } })
  const clear = async () => {
    const names = (await globalThis.caches?.keys().catch(() => [] as string[])) ?? []
    await Promise.all(names.map((name) => globalThis.caches.delete(name)))
    setCleared(true)
    refresh()
  }

  return (
    <div className="tg-storage-settings">
      <section className="tg-storage-settings__card" aria-label="自动下载媒体">
        <h3 className="tg-storage-settings__title">自动下载媒体</h3>
        {NETWORKS.map((network) => {
          const rule = settings.autoDownload[network.id]
          return (
            <div key={network.id} className="tg-storage-settings__network">
              <p className="tg-storage-settings__label">{network.label}</p>
              <Toggle label="图片" checked={rule.photo} onCheckedChange={(photo) => setRule(network.id, { photo })} />
              <Toggle label="视频" checked={rule.video} onCheckedChange={(video) => setRule(network.id, { video })} />
              <Toggle label="文件" checked={rule.file} onCheckedChange={(file) => setRule(network.id, { file })} />
              <label className="tg-storage-settings__row">
                <span>视频与文件上限</span>
                <select
                  value={rule.maxBytes}
                  onChange={(event) => setRule(network.id, { maxBytes: Number(event.target.value) })}
                >
                  {SIZE_CAPS.map((cap) => (
                    <option key={cap.bytes} value={cap.bytes}>
                      {cap.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )
        })}
      </section>
      <section className="tg-storage-settings__card" aria-label="存储占用">
        <h3 className="tg-storage-settings__title">存储占用</h3>
        <p className="tg-storage-settings__row">
          <span>本浏览器已用</span>
          <span>
            {mb(usage?.used)} / {mb(usage?.quota)}
          </span>
        </p>
        <p className="tg-storage-settings__row">
          <span>缓存分区</span>
          <span>{usage?.caches ?? 0}</span>
        </p>
        <Button variant="danger" onClick={() => void clear()}>
          清除缓存
        </Button>
        {cleared ? (
          <p className="tg-storage-settings__hint" role="status">
            已清除。正在查看的媒体不受影响。
          </p>
        ) : null}
        <label className="tg-storage-settings__row">
          <span>最大缓存</span>
          <select
            value={settings.cacheLimitMb}
            onChange={(event) => save({ cacheLimitMb: Number(event.target.value) })}
          >
            {LIMITS.map((limit) => (
              <option key={limit} value={limit}>
                {limit >= 1024 ? `${limit / 1024} GB` : `${limit} MB`}
              </option>
            ))}
          </select>
        </label>
        <label className="tg-storage-settings__row">
          <span>保留时间</span>
          <select
            value={settings.cacheRetentionDays}
            onChange={(event) => save({ cacheRetentionDays: Number(event.target.value) })}
          >
            {RETENTION.map((days) => (
              <option key={days} value={days}>
                {days === 0 ? '永久' : `${days} 天`}
              </option>
            ))}
          </select>
        </label>
      </section>
    </div>
  )
}
