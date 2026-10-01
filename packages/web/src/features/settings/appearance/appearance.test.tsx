/** TG-1204: appearance page choices are shared radio rows (colour parsing: test/cssColor.test.ts). */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { AppearanceSettingsPage } from './AppearanceSettingsPage'

describe('TG-1204 appearance', () => {
  test('theme and wallpaper scope are styled radio rows, not bare native radios', () => {
    const html = renderToStaticMarkup(<AppearanceSettingsPage />)
    expect(html.match(/role="radiogroup"/g)?.length).toBe(2)
    expect(html.match(/class="tg-radio[ "]/g)?.length).toBe(5)
    expect(html).not.toMatch(/<label[^>]*><input type="radio"/)
  })
})
