import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { SocialApi } from '@tg/core'
import { ContactsPage } from './ContactsPage'

describe('TG-702 contacts page', () => {
  test('offers friends, requests, blocklist and add', () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <ContactsPage api={{} as SocialApi} />
      </MemoryRouter>,
    )
    for (const label of ['联系人', '好友 0', '申请 0', '黑名单', '添加']) expect(html).toContain(label)
    expect(html).toContain('aria-selected="true"')
  })
})
