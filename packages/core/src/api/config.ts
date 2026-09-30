/** `GET /api/config` — unauthenticated deployment configuration (TG-011). */
import type { PublicConfig } from '../types'
import type { ApiClient } from './http'

export function getPublicConfig(client: ApiClient): Promise<PublicConfig> {
  return client.json<PublicConfig>('GET', '/api/config')
}
