import { describe, expect, test } from 'bun:test'
import { DEFAULT_SETTINGS } from '../stores/settingsStore'
import { ACCENTS, exportTheme, importTheme, isNightAt, resolveThemeAt } from './appearance'

const at = (hhmm: string) => new Date(`2026-10-01T${hhmm}:00`)

describe('TG-507 appearance rules', () => {
  test('a night window can cross midnight', () => {
    expect(isNightAt('22:00', '07:00', at('23:30'))).toBe(true)
    expect(isNightAt('22:00', '07:00', at('06:59'))).toBe(true)
    expect(isNightAt('22:00', '07:00', at('07:00'))).toBe(false)
    expect(isNightAt('13:00', '15:00', at('14:00'))).toBe(true)
    expect(isNightAt('13:00', '13:00', at('13:00'))).toBe(false)
    expect(isNightAt('bad', '07:00', at('03:00'))).toBe(false)
  })

  test('the theme follows the preference, the OS, or the schedule', () => {
    const schedule = { nightFrom: '22:00', nightTo: '07:00' }
    expect(resolveThemeAt('dark', false, schedule, at('12:00'))).toBe('night')
    expect(resolveThemeAt('light', true, schedule, at('23:00'))).toBe('day')
    expect(resolveThemeAt('system', true, schedule, at('12:00'))).toBe('night')
    expect(resolveThemeAt('scheduled', true, schedule, at('12:00'))).toBe('day')
    expect(resolveThemeAt('scheduled', false, schedule, at('23:00'))).toBe('night')
  })

  test('eight accents; a theme file round-trips and junk is refused', () => {
    expect(ACCENTS).toHaveLength(8)
    const file = exportTheme({ ...DEFAULT_SETTINGS, theme: 'scheduled', accent: 'red', nightFrom: '21:30' })
    expect(importTheme(file)).toEqual({ theme: 'scheduled', accent: 'red', nightFrom: '21:30', nightTo: '07:00' })
    expect(importTheme('{')).toBeNull()
    expect(importTheme(JSON.stringify({ format: 'tg-theme', version: 1, theme: 'neon' }))).toBeNull()
    expect(importTheme(file.replace('"red"', '"chartreuse"'))).toBeNull()
  })
})
