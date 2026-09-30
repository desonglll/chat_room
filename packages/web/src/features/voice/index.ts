/**
 * TG-401 public surface. `register.ts` plugs the bubble body in (side-effect import);
 * `VoiceRecordButton` is the composer's mic button; `applyVoiceListenedFrame` is what the
 * chat session calls for every `voice_listened` frame.
 */
export { VoiceRecordButton } from './VoiceRecordButton'
export type { VoiceRecordButtonProps } from './VoiceRecordButton'
export { VoiceContent, VoiceBody } from './VoiceContent'
export { applyVoiceListenedFrame, createVoiceStore, voiceStore } from './voiceStore'
export type { VoiceStore, VoiceStoreState } from './voiceStore'
