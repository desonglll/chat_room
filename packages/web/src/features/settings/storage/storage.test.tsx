import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { DEFAULT_AUTO_DOWNLOAD, settingsStore } from '@tg/core'
import { AutoDownloadGate } from './AutoDownloadGate'

describe('TG-509 automatic download gate', () => {
  test('media loads when the rule allows it, otherwise a tap-to-download placeholder stands in', () => {
    settingsStore.getState().update({ autoDownload: DEFAULT_AUTO_DOWNLOAD })
    const small = renderToStaticMarkup(
      <AutoDownloadGate kind="video" sizeBytes={1024}>
        <video data-testid="media" />
      </AutoDownloadGate>,
    )
    expect(small).toContain('<video')
    const big = renderToStaticMarkup(
      <AutoDownloadGate kind="video" sizeBytes={200 * 1024 * 1024}>
        <video />
      </AutoDownloadGate>,
    )
    expect(big).not.toContain('<video')
    expect(big).toContain('200.0 MB')
    settingsStore.getState().update({
      autoDownload: { ...DEFAULT_AUTO_DOWNLOAD, wifi: { ...DEFAULT_AUTO_DOWNLOAD.wifi, photo: false } },
    })
    expect(
      renderToStaticMarkup(
        <AutoDownloadGate kind="photo" sizeBytes={1024}>
          <img alt="" />
        </AutoDownloadGate>,
      ),
    ).not.toContain('<img')
    settingsStore.getState().update({ autoDownload: DEFAULT_AUTO_DOWNLOAD })
  })
})
