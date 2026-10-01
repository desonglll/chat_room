/** TG-407: the app-wide locations client and the deployment's map tiles (D-010). */
import { createStore } from 'zustand/vanilla'
import { authStore, createLocationsApi, getPublicConfig, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const locationsApi = createLocationsApi(apiClient, () => selectToken(authStore.getState()) || null)

export const OSM_TILE_URL = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png'
export const OSM_ATTRIBUTION = '© OpenStreetMap contributors'

export interface MapTiles {
  url: string
  attribution: string
  loaded: boolean
}

export const mapTilesStore = createStore<MapTiles>()(() => ({
  url: OSM_TILE_URL,
  attribution: OSM_ATTRIBUTION,
  loaded: false,
}))

/** Reads `/api/config` once; until then (or on an older server) OpenStreetMap is used. */
export function loadMapTiles(): void {
  if (mapTilesStore.getState().loaded) return
  mapTilesStore.setState({ loaded: true })
  void getPublicConfig(apiClient).then(
    (config) =>
      mapTilesStore.setState({
        url: config.map_tile_url || OSM_TILE_URL,
        attribution: config.map_attribution || OSM_ATTRIBUTION,
      }),
    () => undefined,
  )
}

/** A link to the place on openstreetmap.org («在地图中打开»). */
export function osmLink(latitude: number, longitude: number): string {
  return `https://www.openstreetmap.org/?mlat=${latitude}&mlon=${longitude}#map=16/${latitude}/${longitude}`
}
