export interface ComposerKeyEvent {
  key: string
  shiftKey: boolean
  isComposing: boolean
  keyCode: number
}

export function shouldSubmitMessage(
  event: ComposerKeyEvent,
  composing: boolean,
  shortcut: 'enter' | 'shift-enter' = 'enter',
): boolean {
  return event.key === 'Enter'
    && (shortcut === 'shift-enter' ? event.shiftKey : !event.shiftKey)
    && !event.isComposing
    && !composing
    && event.keyCode !== 229
}
