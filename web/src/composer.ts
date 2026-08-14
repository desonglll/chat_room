export interface ComposerKeyEvent {
  key: string
  shiftKey: boolean
  isComposing: boolean
  keyCode: number
}

export function shouldSubmitMessage(
  event: ComposerKeyEvent,
  composing: boolean,
  compositionJustEnded = false,
): boolean {
  return event.key === 'Enter'
    && !event.shiftKey
    && !event.isComposing
    && !composing
    && !compositionJustEnded
    && event.keyCode !== 229
}
