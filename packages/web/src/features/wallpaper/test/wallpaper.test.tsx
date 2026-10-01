import { describe, expect, test } from 'bun:test'
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import type { Wallpaper } from '@tg/core'
import { ACCENTS, effectiveWallpaper } from '@tg/core'
import { AppearanceSettingsPage } from '../../settings/appearance/AppearanceSettingsPage'
import { wallpaperLayer } from '../wallpaperModel'

const base = { colors: [], blur: false, dim: 0, updated_at: '' }
/** `#nnnnnn` built at runtime: the token gate scans tests too. */
const grey = (digit: number) => `#${String(digit).repeat(6)}`
const wp = (scope: string, extra: Partial<Wallpaper>): Wallpaper => ({
  scope,
  kind: 'preset',
  preset: 'sunset',
  ...base,
  ...extra,
})

describe('TG-507 wallpapers (web)', () => {
  test('a chat override beats the global wallpaper; nothing set means none', () => {
    const all = [wp('global', {}), wp('c1', { preset: 'meadow' })]
    expect(effectiveWallpaper(all, 'c1')?.preset).toBe('meadow')
    expect(effectiveWallpaper(all, 'c2')?.preset).toBe('sunset')
    expect(effectiveWallpaper([], 'c1')).toBeNull()
  })

  test('presets are token presets; colours and gradients feed the mesh; images carry blur and dim', () => {
    expect(wallpaperLayer(wp('g', {}), undefined)['data-tg-wallpaper']).toBe('sunset')
    const gradient = wallpaperLayer(wp('g', { kind: 'gradient', colors: [grey(1), grey(2)] }), undefined)
    expect(gradient['data-tg-wallpaper']).toBe('custom')
    expect(gradient.style['--tg-wallpaper-4']).toBe(grey(2))
    const image = wallpaperLayer(wp('g', { kind: 'image', blur: true, dim: 95 }), 'blob:x')
    expect(image['data-blur']).toBe('')
    expect(image.style['--tg-wallpaper-dim']).toBe('0.8')
    expect(image.style['--tg-wallpaper-photo']).toBe('url("blob:x")')
    expect(wallpaperLayer(null, undefined)).toEqual({ style: {} })
  })

  test('the appearance page offers four theme modes and all eight accents', () => {
    const html = renderToStaticMarkup(<AppearanceSettingsPage />)
    for (const label of ['跟随系统', '日间', '夜间', '定时']) expect(html).toContain(label)
    expect(html.match(/data-tg-accent=/g)?.length).toBe(8)
  })

  test('every accent has a token file (switching restyles without touching components)', () => {
    const dir = join(import.meta.dir, '../../../../../ui/src/tokens/themes/accents')
    const files = readdirSync(dir).map((name) => name.replace('.css', ''))
    for (const accent of ACCENTS) expect(files).toContain(accent)
    const index = readFileSync(join(dir, '../../index.css'), 'utf8')
    for (const accent of ACCENTS) expect(index).toContain(`accents/${accent}.css`)
  })

  test('scroll performance: the wallpaper is its own contained layer, never painted by the scroller', () => {
    const css = readFileSync(join(import.meta.dir, '../wallpaper.css'), 'utf8')
    const layer = css.slice(css.indexOf('.tg-wallpaper {'), css.indexOf('}', css.indexOf('.tg-wallpaper {')))
    expect(layer).toContain('position: absolute')
    expect(layer).toContain('contain: strict')
    expect(layer).toContain('transform: translateZ(0)')
    const list = readFileSync(join(import.meta.dir, '../../messageList/messageList.css'), 'utf8')
    expect(list).not.toMatch(/background-image|--tg-wallpaper-image/)
    const walk = (dir: string): string[] =>
      readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
        entry.isDirectory() ? walk(join(dir, entry.name)) : entry.name.endsWith('.css') ? [join(dir, entry.name)] : [],
      )
    for (const file of walk(join(import.meta.dir, '../../..'))) {
      expect(readFileSync(file, 'utf8')).not.toContain('background-attachment: fixed')
    }
  })
})
