/**
 * TG-407 locations: send a static or live location, move or stop a live one, and list the
 * chat's live locations for the merged map. Wire contract in `docs/devlog/TG-407.md`.
 */
import type { LiveLocationEntry, MessageLocation, StoredMessage } from '../types'
import { encodePathSegment, type ApiClient } from './http'
import { t } from '../i18n/t'

/** Telegram's live periods: 15 minutes, 1 hour, 8 hours. */
export const LIVE_LOCATION_PERIODS = [900, 3600, 28_800] as const

export interface LocationPointInput {
  latitude: number
  longitude: number
  accuracy_m?: number
  heading?: number
}

export interface SendLocationInput extends LocationPointInput {
  title?: string
  address?: string
  /** Absent = a static location. */
  live_seconds?: number
  reply_to?: string
  topic_id?: string
}

export interface LocationsApi {
  send(chatId: string, input: SendLocationInput): Promise<StoredMessage>
  /** Rejects with 409 once sharing has ended. */
  move(chatId: string, messageId: string, point: LocationPointInput): Promise<MessageLocation>
  stop(chatId: string, messageId: string): Promise<MessageLocation>
  live(chatId: string): Promise<LiveLocationEntry[]>
}

export function createLocationsApi(client: ApiClient, token: () => string | null): LocationsApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chat = (chatId: string, suffix: string) => `/api/chats/${encodePathSegment(chatId)}${suffix}`
  const live = (chatId: string, messageId: string) => chat(chatId, `/live-locations/${encodePathSegment(messageId)}`)
  return {
    send: (chatId, input) =>
      client.json<StoredMessage>('POST', chat(chatId, '/location-messages'), { ...auth(), body: input }),
    move: (chatId, messageId, point) =>
      client.json<MessageLocation>('PUT', live(chatId, messageId), { ...auth(), body: point }),
    stop: (chatId, messageId) => client.json<MessageLocation>('DELETE', live(chatId, messageId), auth()),
    live: (chatId) => client.json<LiveLocationEntry[]>('GET', chat(chatId, '/live-locations'), auth()),
  }
}

/** Whether a location is still being shared live at `now`. */
export function isLiveLocation(location: MessageLocation, now: number): boolean {
  return location.live_until !== undefined && Date.parse(location.live_until) > now
}

/** Coarsen a point to ~1 km (two decimals) for «模糊位置»; drops accuracy and heading. */
export function approximatePoint(point: LocationPointInput): LocationPointInput {
  const round = (value: number) => Math.round(value * 100) / 100
  return { latitude: round(point.latitude), longitude: round(point.longitude), accuracy_m: 1000 }
}

/** «剩余 12 分钟» / «剩余 3 小时» — what is left of a live share, or '' when it has ended. */
export function liveRemaining(location: MessageLocation, now: number): string {
  if (!location.live_until) return ''
  const left = Date.parse(location.live_until) - now
  if (!(left > 0)) return ''
  const minutes = Math.ceil(left / 60_000)
  return minutes < 60 ? t('c.api.c0a4b1', minutes) : t('c.api.5977b6', Math.ceil(minutes / 60))
}
