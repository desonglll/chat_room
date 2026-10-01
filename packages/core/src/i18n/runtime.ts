/**
 * TG-510 i18n runtime: catalogs per locale, a current locale, and `translate`. Messages are
 * strings with positional `{0}`, `{1}` … slots, or plural tables chosen by the first numeric
 * argument through `Intl.PluralRules` (so each language brings its own plural rule). Platform-free:
 * the host persists the choice and re-renders on `localeStore` changes.
 */
import { createStore } from 'zustand/vanilla'

export type Locale = 'zh-CN' | 'en'
export const LOCALES: readonly { id: Locale; label: string }[] = [
  { id: 'zh-CN', label: '简体中文' },
  { id: 'en', label: 'English' },
]
export const DEFAULT_LOCALE: Locale = 'zh-CN'

export type PluralMessage = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string }
export type Message = string | PluralMessage
export type Catalog = Record<string, Message>

const catalogs: Record<Locale, Catalog> = { 'zh-CN': {}, en: {} }

export const localeStore = createStore<{ locale: Locale }>()(() => ({ locale: DEFAULT_LOCALE }))

export function registerMessages(locale: Locale, catalog: Catalog): void {
  Object.assign(catalogs[locale], catalog)
}

export function setLocale(locale: Locale): void {
  if (localeStore.getState().locale !== locale) localeStore.setState({ locale })
}

export function isLocale(value: unknown): value is Locale {
  return LOCALES.some((locale) => locale.id === value)
}

const pluralRules = new Map<Locale, Intl.PluralRules>()

function pluralCategory(locale: Locale, count: number): Intl.LDMLPluralRule {
  let rules = pluralRules.get(locale)
  if (!rules) {
    rules = new Intl.PluralRules(locale)
    pluralRules.set(locale, rules)
  }
  return rules.select(count)
}

function fill(template: string, args: readonly unknown[]): string {
  return template.replace(/\{(\d+)\}/g, (whole, index: string) => {
    const value = args[Number(index)]
    return value === undefined ? whole : String(value)
  })
}

/**
 * The message `key` in the current locale (falling back to the default locale, then to the
 * key itself), with `{n}` slots filled from `args`. A plural table picks its form by the first
 * numeric argument.
 */
export function translate(key: string, ...args: unknown[]): string {
  const locale = localeStore.getState().locale
  const message = catalogs[locale][key] ?? catalogs[DEFAULT_LOCALE][key]
  if (message === undefined) return key
  if (typeof message === 'string') return fill(message, args)
  const count = args.find((arg): arg is number => typeof arg === 'number') ?? 0
  const form = message[pluralCategory(catalogs[locale][key] ? locale : DEFAULT_LOCALE, count)] ?? message.other
  return fill(form, args)
}

/** Every key of a locale's catalog (for completeness tests). */
export function catalogKeys(locale: Locale): string[] {
  return Object.keys(catalogs[locale])
}
