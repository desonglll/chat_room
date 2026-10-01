import { describe, expect, test } from 'bun:test'
import { DEFAULT_AUTO_DOWNLOAD, networkTypeOf, shouldAutoDownload } from './autoDownload'

const MB = 1024 * 1024

describe('automatic media download', () => {
  test('Telegram defaults: small media on Wi-Fi, photos on mobile data, nothing roaming', () => {
    expect(shouldAutoDownload(DEFAULT_AUTO_DOWNLOAD, 'wifi', 'video', 10 * MB)).toBe(true)
    expect(shouldAutoDownload(DEFAULT_AUTO_DOWNLOAD, 'wifi', 'video', 20 * MB)).toBe(false)
    expect(shouldAutoDownload(DEFAULT_AUTO_DOWNLOAD, 'cellular', 'photo', 5 * MB)).toBe(true)
    expect(shouldAutoDownload(DEFAULT_AUTO_DOWNLOAD, 'cellular', 'video', 100)).toBe(false)
    expect(shouldAutoDownload(DEFAULT_AUTO_DOWNLOAD, 'roaming', 'photo', 100)).toBe(false)
  })

  test('the network type: cellular, save-data counts as roaming, anything else as Wi-Fi', () => {
    expect(networkTypeOf({ type: 'cellular' })).toBe('cellular')
    expect(networkTypeOf({ type: 'wifi', saveData: true })).toBe('roaming')
    expect(networkTypeOf({ type: 'ethernet' })).toBe('wifi')
    expect(networkTypeOf(undefined)).toBe('wifi')
  })
})
