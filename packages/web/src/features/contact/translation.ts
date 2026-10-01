/**
 * TG-410: «翻译». Offered only when the server has an AI provider (checked once per session);
 * otherwise the entry is hidden, never an error. The translation is shown to the viewer who
 * asked and is not written back over the message (a projection, CONTEXT.md).
 */
import { createStore } from 'zustand/vanilla'
import { contactsApi } from './contactsApi'
import { t } from '../../i18n/index'

export interface TranslationState {
  available: boolean
  open: { messageId: string; original: string } | null
  text: string
  loading: boolean
  error: string
}

export const translationStore = createStore<TranslationState>()(() => ({
  available: false,
  open: null,
  text: '',
  loading: false,
  error: '',
}))

let checked = false

/** Ask the server once whether translation exists; the menu entry appears only after `true`. */
export function checkTranslationAvailability(api = contactsApi): void {
  if (checked) return
  checked = true
  void api.translationAvailable().then((available) => translationStore.setState({ available }))
}

/** The viewer's language as a model-friendly name. */
export function viewerLanguage(): string {
  const language = (globalThis.navigator?.language ?? 'zh-CN').toLowerCase()
  if (language.startsWith('zh')) return t('w.contact.936591')
  if (language.startsWith('ja')) return t('w.contact.c12140')
  return 'English'
}

export function translateMessage(messageId: string, original: string, api = contactsApi): void {
  translationStore.setState({ open: { messageId, original }, text: '', loading: true, error: '' })
  api.translate(messageId, viewerLanguage()).then(
    (text) => translationStore.setState({ text, loading: false }),
    () => translationStore.setState({ loading: false, error: t('w.contact.cd2b82') }),
  )
}
