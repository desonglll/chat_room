/** TG-1204: the language page is a settings card of shared radio rows, not bare native radios. */
import { afterEach, describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { setLocale } from '@tg/core'
import { loadLocale } from '../../../i18n/index'
import { LanguageSettingsPage } from './LanguageSettingsPage'

afterEach(() => setLocale('zh-CN'))

describe('TG-1204 language page', () => {
  test('one card, a styled row per language, each named in itself and in the UI language', async () => {
    const zh = renderToStaticMarkup(<LanguageSettingsPage />)
    expect(zh).toContain('class="tg-settings__group"')
    expect(zh.match(/class="tg-radio[ "]/g)?.length).toBe(2)
    expect(zh).toContain('<span lang="en">English</span>')
    expect(zh).toContain('英语')

    await loadLocale('en')
    setLocale('en')
    const en = renderToStaticMarkup(<LanguageSettingsPage />)
    expect(en).toContain('<span lang="zh-CN">简体中文</span>')
    expect(en).toContain('Chinese (Simplified)')
  })
})
