/** User-facing copy for a failed round video recording or upload (TG-402). */
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import { RecorderError } from '../voice/voiceRecorder'
import { t } from '../../i18n/index'

export function videoNoteErrorText(error: unknown): string {
  if (error instanceof RecorderError) {
    if (error.reason === 'insecure') return t('w.videoNote.7660f2')
    if (error.reason === 'unsupported') return t('w.videoNote.a27161')
    if (error.reason === 'permission') return t('w.videoNote.3c2113')
    if (error.reason === 'nodevice') return t('w.videoNote.554443')
    return t('w.videoNote.cf010b')
  }
  if (error instanceof ApiError) {
    if (error.serverMessage === VOICE_RESTRICTED) return t('w.videoNote.c0f9eb')
    if (error.status === 403) return t('w.videoNote.4fcc34')
    if (error.status === 413) return t('w.videoNote.35a826')
  }
  return t('w.videoNote.ea8edf')
}
