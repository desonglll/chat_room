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

const THEMES: { id: ThemePreference; label: string }[] = [
  { id: 'system', label: '跟随系统' },
  { id: 'light', label: '日间' },
  { id: 'dark', label: '夜间' },
  { id: 'scheduled', label: '定时' },
]
const ACCENT_LABEL: Record<AccentId, string> = {
  blue: '蓝',
  cyan: '青',
  green: '绿',
  orange: '橙',
  pink: '粉',
  purple: '紫',
  red: '红',
  gray: '灰',
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
      () => setMessage('已更新聊天背景'),
      () => setMessage('保存失败'),
    )
  const current = wallpapers.find((wallpaper) => wallpaper.scope === scope)

  return (
    <div className="tg-appearance">
      <section className="tg-appearance__card" aria-label="主题">
        <h3 className="tg-appearance__title">主题</h3>
        <div className="tg-appearance__row" role="radiogroup" aria-label="主题">
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
              夜间开始{' '}
              <input
                type="time"
                value={settings.nightFrom}
                onChange={(event) => save({ nightFrom: event.target.value })}
              />
            </label>
            <label>
              结束{' '}
              <input type="time" value={settings.nightTo} onChange={(event) => save({ nightTo: event.target.value })} />
            </label>
          </div>
        ) : null}
      </section>

      <section className="tg-appearance__card" aria-label="强调色">
        <h3 className="tg-appearance__title">强调色</h3>
        <div className="tg-appearance__accents">
          {ACCENTS.map((accent) => (
            <button
              key={accent}
              type="button"
              className="tg-appearance__accent"
              data-tg-accent={accent}
              aria-pressed={settings.accent === accent}
              aria-label={`${ACCENT_LABEL[accent]}色`}
              onClick={() => save({ accent })}
            />
          ))}
        </div>
      </section>

      <section className="tg-appearance__card" aria-label="聊天背景">
        <h3 className="tg-appearance__title">聊天背景</h3>
        <div className="tg-appearance__row">
          <label>
            <input
              type="radio"
              name="scope"
              checked={scope === GLOBAL_WALLPAPER}
              onChange={() => setScope(GLOBAL_WALLPAPER)}
            />{' '}
            所有聊天
          </label>
          {activeChatId ? (
            <label>
              <input
                type="radio"
                name="scope"
                checked={scope === activeChatId}
                onChange={() => setScope(activeChatId)}
              />{' '}
              仅当前聊天
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
              aria-label={`预设背景 ${preset}`}
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
              aria-label={`颜色 ${index + 1}`}
              value={colour || accentHex()}
              onChange={(event) => setColors(colors.map((value, at) => (at === index ? event.target.value : value)))}
            />
          ))}
          {colors.length < 4 ? (
            <Button variant="text" size="sm" onClick={() => setColors([...colors, colors[colors.length - 1] ?? ''])}>
              + 颜色
            </Button>
          ) : null}
          <Button size="sm" onClick={() => apply({ kind: 'color', colors: [colors[0] || accentHex()] })}>
            纯色
          </Button>
          <Button
            size="sm"
            onClick={() => apply({ kind: 'gradient', colors: colors.map((value) => value || accentHex()) })}
          >
            渐变
          </Button>
        </div>
        <Toggle label="模糊（图片背景）" checked={blur} onCheckedChange={setBlur} />
        <label className="tg-appearance__row">
          暗化 {dim}%
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
            上传图片
            <input
              type="file"
              accept="image/*"
              hidden
              onChange={(event) => {
                const file = event.target.files?.[0]
                event.target.value = ''
                if (file)
                  void uploadWallpaper(scope, file, { blur, dim }).then(
                    () => setMessage('已更新聊天背景'),
                    () => setMessage('上传失败'),
                  )
              }}
            />
          </label>
          <Button
            variant="text"
            size="sm"
            onClick={() => void resetWallpaper(scope).then(() => setMessage('已恢复默认'))}
          >
            恢复默认
          </Button>
        </div>
      </section>

      <section className="tg-appearance__card" aria-label="自定义主题">
        <h3 className="tg-appearance__title">自定义主题</h3>
        <TextField
          multiline
          rows={4}
          aria-label="主题文件"
          placeholder="粘贴主题文件，或点「导出」得到当前主题"
          value={themeText}
          onChange={(event) => setThemeText(event.target.value)}
        />
        <div className="tg-appearance__row">
          <Button size="sm" variant="tonal" onClick={() => setThemeText(exportTheme(settings))}>
            导出
          </Button>
          <Button
            size="sm"
            onClick={() => {
              const imported = importTheme(themeText)
              if (imported) {
                save(imported)
                setMessage('主题已导入')
              } else setMessage('不是有效的主题文件')
            }}
          >
            导入
          </Button>
        </div>
      </section>
      {message ? <p role="status">{message}</p> : null}
    </div>
  )
}
