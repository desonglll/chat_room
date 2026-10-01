/**
 * TG-507 appearance rules: which theme applies now (incl. the night schedule), the accent list,
 * and the portable custom-theme format (export/import). Pure; the web app applies the result
 * as `data-tg-theme` / `data-tg-accent`, so no component changes when the theme does.
 */
import type { AccentId, SettingsSnapshot, ThemePreference } from '../stores/settingsStore'

export const ACCENTS: readonly AccentId[] = ['blue', 'cyan', 'green', 'orange', 'pink', 'purple', 'red', 'gray']

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/

function minutes(value: string): number | null {
  const match = TIME.exec(value)
  return match ? Number(match[1]) * 60 + Number(match[2]) : null
}

/** Whether `at` (local time) falls in the night window; a window may cross midnight. */
export function isNightAt(from: string, to: string, at: Date): boolean {
  const start = minutes(from)
  const end = minutes(to)
  if (start === null || end === null || start === end) return false
  const now = at.getHours() * 60 + at.getMinutes()
  return start < end ? now >= start && now < end : now >= start || now < end
}

export function resolveThemeAt(
  preference: ThemePreference,
  systemPrefersDark: boolean,
  schedule: { nightFrom: string; nightTo: string },
  at: Date,
): 'day' | 'night' {
  if (preference === 'dark') return 'night'
  if (preference === 'light') return 'day'
  if (preference === 'scheduled') return isNightAt(schedule.nightFrom, schedule.nightTo, at) ? 'night' : 'day'
  return systemPrefersDark ? 'night' : 'day'
}

/** The shareable part of the appearance: a small JSON document. */
export interface ThemeFile {
  format: 'tg-theme'
  version: 1
  theme: ThemePreference
  accent: AccentId
  nightFrom: string
  nightTo: string
}

export function exportTheme(settings: Pick<SettingsSnapshot, 'theme' | 'accent' | 'nightFrom' | 'nightTo'>): string {
  const file: ThemeFile = {
    format: 'tg-theme',
    version: 1,
    theme: settings.theme,
    accent: settings.accent,
    nightFrom: settings.nightFrom,
    nightTo: settings.nightTo,
  }
  return JSON.stringify(file, null, 2)
}

const THEMES: readonly ThemePreference[] = ['light', 'dark', 'system', 'scheduled']

/** The settings a theme file sets, or `null` when it is not a valid theme file. */
export function importTheme(text: string): Pick<SettingsSnapshot, 'theme' | 'accent' | 'nightFrom' | 'nightTo'> | null {
  let parsed: Partial<ThemeFile>
  try {
    parsed = JSON.parse(text) as Partial<ThemeFile>
  } catch {
    return null
  }
  if (parsed?.format !== 'tg-theme' || parsed.version !== 1) return null
  const { theme, accent, nightFrom, nightTo } = parsed
  if (!theme || !THEMES.includes(theme) || !accent || !ACCENTS.includes(accent)) return null
  if (typeof nightFrom !== 'string' || typeof nightTo !== 'string' || !TIME.test(nightFrom) || !TIME.test(nightTo)) {
    return null
  }
  return { theme, accent, nightFrom, nightTo }
}
