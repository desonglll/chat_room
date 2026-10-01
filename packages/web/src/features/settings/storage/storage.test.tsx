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
    // Rule changes are covered by core's `autoDownload.test.ts`: static rendering reads the
    // store's initial state, so it cannot observe an update here.
    settingsStore.getState().update({ autoDownload: DEFAULT_AUTO_DOWNLOAD })
  })
})
