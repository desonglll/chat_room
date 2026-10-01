/** TG-1204: appearance page — colour inputs start at the accent, choices are shared radio rows. */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { AppearanceSettingsPage } from './AppearanceSettingsPage'
import { toHexColor } from './cssColor'

describe('TG-1204 appearance', () => {
  test('a minified or rgb() accent still becomes a value <input type="color"> accepts', () => {
    expect(toHexColor('#08f')).toBe('#0088ff')
    expect(toHexColor(' #3390EC ')).toBe('#3390ec')
    expect(toHexColor('#3390ec80')).toBe('#3390ec')
    expect(toHexColor('#08f8')).toBe('#0088ff')
    expect(toHexColor('rgb(51, 144, 236)')).toBe('#3390ec')
    expect(toHexColor('rgba(51 144 236 / 50%)')).toBe('#3390ec')
    expect(toHexColor('var(--x)')).toBe('')
  })

  test('theme and wallpaper scope are styled radio rows, not bare native radios', () => {
    const html = renderToStaticMarkup(<AppearanceSettingsPage />)
    expect(html.match(/role="radiogroup"/g)?.length).toBe(2)
    expect(html.match(/class="tg-radio[ "]/g)?.length).toBe(5)
    expect(html).not.toMatch(/<label[^>]*><input type="radio"/)
  })
})
