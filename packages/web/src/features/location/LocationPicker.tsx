/**
 * TG-1301: choosing a point on the map when the device cannot say where it is — plain http on a
 * LAN address (no geolocation at all), a denied permission, or no fix. Telegram's picker: the pin
 * stays at the centre and the map moves under it (drag, zoom, or tap a spot to bring it to the
 * centre), so a finger never has to grab a small marker. `onChange` reports the centre.
 */
import { useEffect, useRef } from 'react'
import type { LocationPointInput } from '@tg/core'
import { useStore } from 'zustand/react'
import { loadMapTiles, mapTilesStore } from './locationApi'
import { t } from '../../i18n/index'

/** Where the picker opens without any fix: a whole-continent view the user zooms into. */
export const PICKER_START = { latitude: 30, longitude: 110, zoom: 4 }

/** A map centre as a point to send, rounded to ~1 m (6 decimals). */
export function pickedPoint(latitude: number, longitude: number): LocationPointInput {
  const round = (value: number) => Math.round(value * 1e6) / 1e6
  // Leaflet lets the view wrap around the antimeridian; the server wants -180..180.
  const wrapped = ((((longitude + 180) % 360) + 360) % 360) - 180
  return { latitude: round(Math.max(-90, Math.min(90, latitude))), longitude: round(wrapped) }
}

export function LocationPicker({ onChange }: { onChange(point: LocationPointInput): void }) {
  const host = useRef<HTMLDivElement>(null)
  const tiles = useStore(mapTilesStore)
  const report = useRef(onChange)
  report.current = onChange

  useEffect(loadMapTiles, [])

  useEffect(() => {
    let cancelled = false
    let remove: (() => void) | null = null
    void import('./leafletRuntime').then(({ default: L }) => {
      const element = host.current
      if (cancelled || !element) return
      const map = L.map(element, { attributionControl: true }).setView(
        [PICKER_START.latitude, PICKER_START.longitude],
        PICKER_START.zoom,
      )
      L.tileLayer(tiles.url, { attribution: tiles.attribution, maxZoom: 19 }).addTo(map)
      const emit = () => {
        const centre = map.getCenter()
        report.current(pickedPoint(centre.lat, centre.lng))
      }
      map.on('move', emit)
      map.on('click', (event) => map.panTo(event.latlng))
      emit()
      remove = () => map.remove()
    })
    return () => {
      cancelled = true
      remove?.()
    }
  }, [tiles.url, tiles.attribution])

  return (
    <div className="tg-location-picker">
      <div ref={host} className="tg-location-picker__map" role="application" aria-label={t('w.location.175289')} />
      <span className="tg-location-picker__pin" aria-hidden="true" />
    </div>
  )
}
