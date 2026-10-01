import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { messageContent } from '../../message/content/messageContent'
import { makeMessage } from '../../message/fixtures/bubbleFixtures'
import { LocationContent } from '../LocationContent'
import { osmLink } from '../locationApi'
import { applyLocationFrame, effectiveLocation, liveLocationStore } from '../liveLocationStore'
import '../register'

const point = { latitude: 31.23, longitude: 121.47, updated_at: '2026-10-01T12:00:00Z' }

describe('TG-407 locations (web)', () => {
  test('location messages resolve to the location bubble', () => {
    expect(messageContent.resolve(makeMessage({ location: point, media_kind: 'location' }))?.kind).toBe('location')
  })

  test('a newer live point wins over the message snapshot, an older one does not', () => {
    const newer = { ...point, latitude: 1, updated_at: '2026-10-01T12:05:00Z' }
    const older = { ...point, latitude: 2, updated_at: '2026-10-01T11:55:00Z' }
    expect(effectiveLocation(point, newer).latitude).toBe(1)
    expect(effectiveLocation(point, older).latitude).toBe(31.23)
    expect(effectiveLocation(point, undefined)).toBe(point)
    applyLocationFrame({ type: 'location_updated', message_id: 'm1', location: newer })
    expect(liveLocationStore.getState().points.m1?.latitude).toBe(1)
  })

  test('the bubble shows the venue and whether a live share has ended', () => {
    const render = (location: typeof point & { title?: string; live_until?: string }) =>
      renderToStaticMarkup(
        <LocationContent
          message={makeMessage({ location })}
          ctx={{ isOutgoing: false } as never}
          actions={{} as never}
          metaSpacer={null}
        />,
      )
    expect(render({ ...point, title: '外滩' })).toContain('外滩')
    expect(render({ ...point, live_until: '2000-01-01T00:00:00Z' })).toContain('实时位置已结束')
    expect(render(point)).toContain('role="img"')
  })

  test('«在地图中打开» points at openstreetmap.org', () => {
    expect(osmLink(1.5, 2.5)).toBe('https://www.openstreetmap.org/?mlat=1.5&mlon=2.5#map=16/1.5/2.5')
  })
})
