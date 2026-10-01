/**
 * TG-407: a Leaflet map (D-010) with one dot per point. Leaflet loads on first use; tiles come
 * from the deployment's configured template. Dots are drawn as circle markers coloured from
 * the theme tokens (no image assets, nothing to break with a bundler).
 */
import { useEffect, useRef } from 'react'
import { useStore } from 'zustand/react'
import { loadMapTiles, mapTilesStore } from './locationApi'

export interface MapPoint {
  id: string
  latitude: number
  longitude: number
  label: string
  accuracy_m?: number | undefined
}

type Leaflet = typeof import('leaflet')

export function LocationMap({
  points,
  interactive = false,
  className,
}: {
  points: readonly MapPoint[]
  interactive?: boolean
  className?: string
}) {
  const host = useRef<HTMLDivElement>(null)
  const tiles = useStore(mapTilesStore)
  const map = useRef<{ L: Leaflet; map: import('leaflet').Map; layer: import('leaflet').LayerGroup } | null>(null)

  useEffect(loadMapTiles, [])

  useEffect(() => {
    let cancelled = false
    void import('./leafletRuntime').then(({ default: L }) => {
      const element = host.current
      if (cancelled || !element || map.current) return
      const instance = L.map(element, {
        zoomControl: interactive,
        dragging: interactive,
        scrollWheelZoom: interactive,
        doubleClickZoom: interactive,
        touchZoom: interactive,
        boxZoom: interactive,
        keyboard: interactive,
        attributionControl: true,
      })
      L.tileLayer(tiles.url, { attribution: tiles.attribution, maxZoom: 19 }).addTo(instance)
      map.current = { L, map: instance, layer: L.layerGroup().addTo(instance) }
      draw()
    })
    return () => {
      cancelled = true
      map.current?.map.remove()
      map.current = null
    }
    // The tile template is read once per map; a later config change applies to new maps.
  }, [interactive, tiles.url, tiles.attribution])

  const draw = () => {
    const current = map.current
    const element = host.current
    if (!current || !element || points.length === 0) return
    const accent = getComputedStyle(element).getPropertyValue('--tg-accent').trim()
    const surface = getComputedStyle(element).getPropertyValue('--tg-surface').trim()
    current.layer.clearLayers()
    for (const point of points) {
      const at: [number, number] = [point.latitude, point.longitude]
      if (point.accuracy_m && point.accuracy_m > 30) {
        current.L.circle(at, { radius: point.accuracy_m, color: accent, weight: 1, fillOpacity: 0.12 }).addTo(
          current.layer,
        )
      }
      current.L.circleMarker(at, { radius: 8, color: surface, weight: 3, fillColor: accent, fillOpacity: 1 })
        .bindTooltip(point.label)
        .addTo(current.layer)
    }
    if (points.length === 1) {
      const only = points[0]!
      current.map.setView([only.latitude, only.longitude], 15)
    } else {
      current.map.fitBounds(
        current.L.latLngBounds(points.map((point) => [point.latitude, point.longitude] as [number, number])),
        { padding: [24, 24], maxZoom: 16 },
      )
    }
  }

  const key = points.map((point) => `${point.id}:${point.latitude},${point.longitude}`).join('|')
  useEffect(draw, [key])

  return (
    <div
      ref={host}
      className={className ?? 'tg-location-map'}
      role="img"
      aria-label={points.map((p) => p.label).join('，')}
    />
  )
}
