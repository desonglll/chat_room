import { describe, expect, test } from 'bun:test'
import { locationBlockedReason, locationFailureReason } from '../locationAccess'

describe('TG-1301 location access copy', () => {
  test('an insecure page says HTTPS is needed, before and instead of asking for permission', () => {
    // Insecure contexts keep `geolocation`; asking would only yield PERMISSION_DENIED.
    const reason = locationBlockedReason({ isSecureContext: false, navigator: { geolocation: {} } })
    expect(reason).toContain('HTTPS')
    expect(reason).not.toBe(locationFailureReason({ code: 1 }))
  })

  test('a secure page with no geolocation says the browser lacks it; with it, asking may proceed', () => {
    expect(locationBlockedReason({ isSecureContext: true, navigator: {} })).toBe('此浏览器不支持定位')
    expect(locationBlockedReason({ isSecureContext: true, navigator: { geolocation: {} } })).toBeNull()
  })

  test('a denied request and other failures each have their own copy', () => {
    expect(locationFailureReason({ code: 1 })).toBe('未获得定位权限')
    expect(locationFailureReason({ code: 2 })).toBe('无法获取当前位置')
    expect(locationFailureReason({ code: 3 })).toBe('无法获取当前位置')
  })
})
