import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { NotificationsApi } from '@tg/core'
import { NotificationsPage } from './NotificationsPage'
import { applyAccountSignal, notificationsStore } from './notificationsStore'

describe('TG-703 notification center (web)', () => {
  test('account-socket signals update the live counters; other frames are ignored', () => {
    applyAccountSignal({ type: 'notifications_changed', unread_count: 4, latest_notification_id: 'n9' })
    applyAccountSignal({ type: 'social_changed', incoming_request_count: 2 })
    applyAccountSignal({ type: 'something_else', unread_count: 99 })
    expect(notificationsStore.getState()).toMatchObject({ unread: 4, incomingRequests: 2 })
    notificationsStore.setState({ unread: 0, incomingRequests: 0 })
  })

  test('the page has its title and the mark-all-read action', () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <NotificationsPage api={{} as NotificationsApi} />
      </MemoryRouter>,
    )
    expect(html).toContain('通知')
    expect(html).toContain('全部已读')
  })
})
