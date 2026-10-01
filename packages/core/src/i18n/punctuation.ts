/**
 * TG-1204: locale-aware punctuation. Chinese copy uses full-width marks (`张三：…`, `甲，乙`),
 * English uses ASCII ones; a mark hard-coded in a component shows the wrong one in the other
 * locale. Importing these also guarantees core's catalogs are registered (core is
 * `sideEffects: false`, so `t.ts` only runs when something imports it).
 */
import { t } from './t'

/** `name：` / `name: ` — a sender or label prefix. */
export const speaker = (name: string): string => t('c.text.speaker', name)

/** `label：value` / `label: value`. */
export const labelled = (label: string, value: string): string => t('c.text.labelled', label, value)

/** Items joined with the locale's list separator. */
export const joinList = (items: readonly string[]): string => items.join(t('c.text.listSeparator'))
