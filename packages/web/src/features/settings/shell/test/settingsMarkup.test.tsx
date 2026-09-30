/** TG-110: the settings views render (static markup, no DOM renderer is installed). */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ProfileCard, RootView, SectionView, sectionById, type ViewContext } from '../SettingsViews'
import type { SettingsPageRegistration } from '../settingsRegistry'

const Page = () => null
const context = (pages: SettingsPageRegistration[] = []): ViewContext => ({
  pages,
  push: () => undefined,
  back: () => undefined,
  close: () => undefined,
})

test('root: close button, edit button and every section row', () => {
  const html = renderToStaticMarkup(<RootView context={context()} />)
  expect(html).toContain('aria-label="关闭设置"')
  expect(html).toContain('aria-label="编辑资料"')
  for (const title of ['我的账号', '通知与声音', '隐私与安全', '数据与存储', '外观', '语言', '设备']) {
    expect(html).toContain(title)
  }
})

test('profile card: name, @username, and it opens the profile editor', () => {
  const user = {
    id: 'u1',
    username: 'alice',
    avatar_emoji: '🦊',
    display_name: 'Alice',
    signature: '',
    homepage: '',
    created_at: '2026-01-01T00:00:00Z',
  }
  const html = renderToStaticMarkup(<ProfileCard user={user} onEdit={() => undefined} />)
  expect(html).toContain('Alice')
  expect(html).toContain('@alice')
  expect(html).toContain('aria-label="编辑资料：Alice"')
})

test('a section without pages says «即将推出»; with pages it lists them', () => {
  const empty = renderToStaticMarkup(<SectionView section={sectionById('storage')} context={context()} />)
  expect(empty).toContain('即将推出')
  const pages: SettingsPageRegistration[] = [
    { id: 'p.rules', section: 'privacy', title: '隐私', component: Page },
    { id: 'p.2fa', section: 'privacy', title: '两步验证', component: Page },
  ]
  const listed = renderToStaticMarkup(<SectionView section={sectionById('privacy')} context={context(pages)} />)
  expect(listed).not.toContain('即将推出')
  expect(listed).toContain('两步验证')
  expect(listed).toContain('aria-label="返回"')
})
