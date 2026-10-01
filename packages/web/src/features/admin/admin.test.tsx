import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { AdminApi, AdminOverview, SocialApi } from '@tg/core'
import { AdminOverviewCard } from './AdminOverviewCard'
import { AdminPage } from './AdminPage'

const overview: AdminOverview = {
  generated_at: '',
  database_backend: 'sqlite',
  attachment_backend: 'local',
  online_users: 3,
  websocket_connections: 4,
  orphan_retention_hours: 24,
  deleted_room_retention_days: 30,
  chat_rooms_locked: false,
  runtime: {
    uptime_seconds: 7200,
    requests: 100,
    failures: 0,
    active_requests: 1,
    average_latency_ms: 2.5,
    max_latency_ms: 9,
  },
  totals: {
    users: 12,
    active_sessions: 5,
    active_rooms: 7,
    soft_deleted_rooms: 0,
    messages: 900,
    messages_24h: 40,
    attachments: 2,
    attachments_24h: 1,
    pending_uploads: 0,
  },
  storage: { logical_bytes: 2048, physical_bytes: 1536, orphaned_attachments: 0, orphaned_bytes: 0, missing_hashes: 0 },
  services: { items: [{ id: 'redis', label: 'Redis', state: 'healthy', latency_ms: 1, detail: '' }] },
  top_rooms: [],
}

describe('TG-705 admin console', () => {
  test('the overview shows totals, storage, backends and service health', () => {
    const html = renderToStaticMarkup(<AdminOverviewCard overview={overview} />)
    for (const text of ['12', '1.5 KB', 'sqlite', 'Redis', 'data-state="healthy"']) expect(html).toContain(text)
  })

  test('the page starts by checking access', () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <AdminPage api={{} as AdminApi} social={{} as SocialApi} />
      </MemoryRouter>,
    )
    expect(html).toContain('管理后台')
    expect(html).toContain('正在加载')
  })
})
