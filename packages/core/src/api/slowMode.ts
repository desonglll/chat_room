/**
 * TG-207 slow mode. The server enforces the interval on every send path; the client only
 * mirrors it as a countdown. `SLOW_MODE_CHOICES` is Telegram's list (seconds, 0 = off).
 */
import type { Chat } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

export const SLOW_MODE_CHOICES = [0, 10, 30, 60, 300, 900, 3600] as const

export interface SlowModeState {
  seconds: number
  /** Seconds the caller still has to wait; 0 = may send now (always 0 when exempt). */
  wait_seconds: number
  /** Owners and administrators are not subject to slow mode. */
  exempt: boolean
}

export interface SlowModeApi {
  get(chatId: string): Promise<SlowModeState>
  /** Requires `members.ban`; a group with slow mode becomes a supergroup. */
  set(chatId: string, seconds: number): Promise<Chat>
}

export function createSlowModeApi(client: ApiClient, token: () => string | null): SlowModeApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const path = (chatId: string) => `/api/chats/${encodePathSegment(chatId)}/slow-mode`
  return {
    get: (chatId) => client.json<SlowModeState>('GET', path(chatId), auth()),
    set: (chatId, seconds) => client.json<Chat>('PUT', path(chatId), { ...auth(), body: { seconds } }),
  }
}

/** Telegram's labels: «关闭», «10 秒», «1 分钟», «1 小时». */
export function slowModeLabel(seconds: number): string {
  if (seconds <= 0) return '关闭'
  if (seconds < 60) return `${seconds} 秒`
  if (seconds < 3600) return `${seconds / 60} 分钟`
  return `${seconds / 3600} 小时`
}

/** The countdown text: «0:42», «4:05», «59:59». */
export function slowModeCountdown(remainingSeconds: number): string {
  const total = Math.max(0, Math.ceil(remainingSeconds))
  const minutes = Math.floor(total / 60)
  const seconds = total % 60
  return `${minutes}:${String(seconds).padStart(2, '0')}`
}
