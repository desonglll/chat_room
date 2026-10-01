/**
 * TG-510: the web app's `t`. Importing it registers the zh-CN source catalog; core registers
 * its own. Components call `t()` while rendering, and the app root re-renders on
 * `localeStore` changes, so switching language needs no page reload.
 *
 * TG-806: English is a separate chunk loaded on demand (`loadLocale`) — a Chinese-first
 * client should not download every English string on first paint.
 */
import type { Locale } from '@tg/core'
import { registerMessages, translate } from '@tg/core'
import { zh } from './zh/index'

registerMessages('zh-CN', zh)

const loaded = new Map<Locale, Promise<void>>([['zh-CN', Promise.resolve()]])

/** Registers `locale`'s web catalog once; resolves when `t()` can render it. */
export function loadLocale(locale: Locale): Promise<void> {
  let pending = loaded.get(locale)
  if (!pending) {
    pending = import('./en/index').then(({ en }) => registerMessages('en', en))
    loaded.set(locale, pending)
  }
  return pending
}

export const t = translate
