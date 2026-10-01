/**
 * TG-507 «外观»: day/night/system/scheduled with the night window, the accent (eight Telegram
 * accents), the chat wallpaper for all chats or the open chat (presets, one colour, a gradient,
 * or an uploaded image with blur and dim), and custom-theme export/import. Theme and accent are
 * per device; wallpapers are per account (server).
 */
import { useEffect, useState } from 'react'
import type { AccentId, ThemePreference, WallpaperWrite } from '@tg/core'
import { ACCENTS, exportTheme, GLOBAL_WALLPAPER, importTheme, settingsStore, uiStore } from '@tg/core'
import { Button, TextField, Toggle } from '@tg/ui'
import { useStore } from 'zustand/react'
import { browserStorage } from '../../../app/platform'
import {
  loadWallpapers,
  resetWallpaper,
  saveWallpaper,
  uploadWallpaper,
  wallpaperStore,
} from '../../wallpaper/wallpaperStore'
import { t } from '../../../i18n/index'

const THEMES: { id: ThemePreference; label: string }[] = [
  {
    id: 'system',
    get label() {
      return t('w.settings.f4bbd9')
    },
  },
  {
    id: 'light',
    get label() {
      return t('w.settings.e4c5fb')
    },
  },
  {
    id: 'dark',
    get label() {
      return t('w.settings.7dcb90')
    },
  },
  {
    id: 'scheduled',
    get label() {
      return t('w.settings.4c02a6')
    },
  },
]
const ACCENT_LABEL: Record<AccentId, string> = {
  get blue() {
    return t('w.settings.a44c59')
  },
  get cyan() {
    return t('w.settings.845d1d')
  },
  get green() {
    return t('w.settings.579625')
  },
  get orange() {
    return t('w.settings.a01503')
  },
  get pink() {
    return t('w.settings.febcf8')
  },
  get purple() {
    return t('w.settings.01caea')
  },
  get red() {
    return t('w.settings.5e3127')
  },
  get gray() {
    return t('w.settings.c01083')
  },
}
const PRESETS = [
  'default',
  'sunset',
  'meadow',
  'orchid',
  'lagoon',
  'haze',
  'midnight-blue',
  'deep-teal',
  'pine',
  'plum-night',
  'ember',
  'mulberry',
  'forest-night',
  'none',
]

/** The current accent as a `#rrggbb` start value for the colour inputs. */
function accentHex(): string {
  const value = globalThis.getComputedStyle?.(document.documentElement).getPropertyValue('--tg-accent').trim() ?? ''
  return /^#[0-9a-f]{6}$/i.test(value) ? value : ''
}

export function AppearanceSettingsPage() {
  const settings = useStore(settingsStore)
  const activeChatId = useStore(uiStore, (state) => state.activeChatId)
  const wallpapers = useStore(wallpaperStore, (state) => state.wallpapers)
  const [scope, setScope] = useState(GLOBAL_WALLPAPER)
  const [colors, setColors] = useState<string[]>(() => [accentHex(), accentHex()])
  const [blur, setBlur] = useState(false)
  const [dim, setDim] = useState(0)
  const [themeText, setThemeText] = useState('')
  const [message, setMessage] = useState('')
  useEffect(loadWallpapers, [])

  const save = (change: Partial<typeof settings>) => {
    settingsStore.getState().update(change)
    settingsStore.getState().persist(browserStorage)
  }
  const apply = (write: WallpaperWrite) =>
    void saveWallpaper(scope, { ...write, blur, dim }).then(
      () => setMessage(t('w.settings.ac4668')),
      () => setMessage(t('w.settings.40525a')),
    )
  const current = wallpapers.find((wallpaper) => wallpaper.scope === scope)

  return (
    <div className="tg-appearance">
      <section className="tg-appearance__card" aria-label={t('w.settings.e848dd')}>
        <h3 className="tg-appearance__title">{t('w.settings.e848dd')}</h3>
        <div className="tg-appearance__row" role="radiogroup" aria-label={t('w.settings.e848dd')}>
          {THEMES.map((theme) => (
            <label key={theme.id}>
              <input
                type="radio"
                name="theme"
                checked={settings.theme === theme.id}
                onChange={() => save({ theme: theme.id })}
              />{' '}
              {theme.label}
            </label>
          ))}
        </div>
        {settings.theme === 'scheduled' ? (
          <div className="tg-appearance__row">
            <label>
              {t('w.settings.c6abd1')}{' '}
              <input
                type="time"
                value={settings.nightFrom}
                onChange={(event) => save({ nightFrom: event.target.value })}
              />
            </label>
            <label>
              {t('w.settings.76b988')}{' '}
              <input type="time" value={settings.nightTo} onChange={(event) => save({ nightTo: event.target.value })} />
            </label>
          </div>
        ) : null}
      </section>

      <section className="tg-appearance__card" aria-label={t('w.settings.6f6737')}>
        <h3 className="tg-appearance__title">{t('w.settings.6f6737')}</h3>
        <div className="tg-appearance__accents">
          {ACCENTS.map((accent) => (
            <button
              key={accent}
              type="button"
              className="tg-appearance__accent"
              data-tg-accent={accent}
              aria-pressed={settings.accent === accent}
              aria-label={t('w.settings.b4375f', ACCENT_LABEL[accent])}
              onClick={() => save({ accent })}
            />
          ))}
        </div>
      </section>

      <section className="tg-appearance__card" aria-label={t('w.settings.7e9e15')}>
        <h3 className="tg-appearance__title">{t('w.settings.7e9e15')}</h3>
        <div className="tg-appearance__row">
          <label>
            <input
              type="radio"
              name="scope"
              checked={scope === GLOBAL_WALLPAPER}
              onChange={() => setScope(GLOBAL_WALLPAPER)}
            />{' '}
            {t('w.settings.c082c4')}
          </label>
          {activeChatId ? (
            <label>
              <input
                type="radio"
                name="scope"
                checked={scope === activeChatId}
                onChange={() => setScope(activeChatId)}
              />{' '}
              {t('w.settings.89297a')}
            </label>
          ) : null}
        </div>
        <div className="tg-appearance__presets">
          {PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              className="tg-appearance__preset"
              data-tg-wallpaper={preset}
              aria-label={t('w.settings.945793', preset)}
              aria-pressed={current?.kind === 'preset' && current.preset === preset}
              onClick={() => apply({ kind: 'preset', preset })}
            />
          ))}
        </div>
        <div className="tg-appearance__row">
          {colors.map((colour, index) => (
            <input
              key={index}
              type="color"
              aria-label={t('w.settings.7ae353', index + 1)}
              value={colour || accentHex()}
              onChange={(event) => setColors(colors.map((value, at) => (at === index ? event.target.value : value)))}
            />
          ))}
          {colors.length < 4 ? (
            <Button variant="text" size="sm" onClick={() => setColors([...colors, colors[colors.length - 1] ?? ''])}>
              {t('w.settings.4b5093')}
            </Button>
          ) : null}
          <Button size="sm" onClick={() => apply({ kind: 'color', colors: [colors[0] || accentHex()] })}>
            {t('w.settings.c502e4')}
          </Button>
          <Button
            size="sm"
            onClick={() => apply({ kind: 'gradient', colors: colors.map((value) => value || accentHex()) })}
          >
            {t('w.settings.204be2')}
          </Button>
        </div>
        <Toggle label={t('w.settings.765198')} checked={blur} onCheckedChange={setBlur} />
        <label className="tg-appearance__row">
          {t('w.settings.4ecf62')} {dim}%
          <input
            type="range"
            min={0}
            max={80}
            step={5}
            value={dim}
            onChange={(event) => setDim(Number(event.target.value))}
          />
        </label>
        <div className="tg-appearance__row">
          <label className="tg-appearance__upload">
            {t('w.settings.59b308')}
            <input
              type="file"
              accept="image/*"
              hidden
              onChange={(event) => {
                const file = event.target.files?.[0]
                event.target.value = ''
                if (file)
                  void uploadWallpaper(scope, file, { blur, dim }).then(
                    () => setMessage(t('w.settings.ac4668')),
                    () => setMessage(t('w.settings.a6f805')),
                  )
              }}
            />
          </label>
          <Button
            variant="text"
            size="sm"
            onClick={() => void resetWallpaper(scope).then(() => setMessage(t('w.settings.4c82d8')))}
          >
            {t('w.settings.a19193')}
          </Button>
        </div>
      </section>

      <section className="tg-appearance__card" aria-label={t('w.settings.76e1a9')}>
        <h3 className="tg-appearance__title">{t('w.settings.76e1a9')}</h3>
        <TextField
          multiline
          rows={4}
          aria-label={t('w.settings.d8b344')}
          placeholder={t('w.settings.77569c')}
          value={themeText}
          onChange={(event) => setThemeText(event.target.value)}
        />
        <div className="tg-appearance__row">
          <Button size="sm" variant="tonal" onClick={() => setThemeText(exportTheme(settings))}>
            {t('w.settings.188896')}
          </Button>
          <Button
            size="sm"
            onClick={() => {
              const imported = importTheme(themeText)
              if (imported) {
                save(imported)
                setMessage(t('w.settings.dda5af'))
              } else setMessage(t('w.settings.43c3a8'))
            }}
          >
            {t('w.settings.60e2bc')}
          </Button>
        </div>
      </section>
      {message ? <p role="status">{message}</p> : null}
    </div>
  )
}
