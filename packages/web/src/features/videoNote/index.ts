/**
 * TG-402 public surface. `register.ts` plugs the round bubble in (side-effect import);
 * `RecordModeButton` is the composer's mic ↔ camera button (same props as TG-401's
 * `VoiceRecordButton`, which it wraps).
 */
export { RecordModeButton } from './RecordModeButton'
export type { RecordModeButtonProps, RecordMode } from './RecordModeButton'
export { VideoNoteContent } from './VideoNoteContent'
export { VideoNoteBody } from './VideoNoteBody'
export { sendVideoNote, markVideoNoteListened, VIDEO_NOTE_MAX_MS } from './videoNoteApi'
