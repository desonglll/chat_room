export interface ComposerKeyEvent {
  key: string
  shiftKey: boolean
  isComposing: boolean
  keyCode: number
}

export interface FocusKeyEvent {
  key: string
  repeat: boolean
  metaKey: boolean
  ctrlKey: boolean
  altKey: boolean
}

export function shouldSubmitMessage(
  event: ComposerKeyEvent,
  composing: boolean,
  shortcut: 'enter' | 'shift-enter' = 'enter',
): boolean {
  return (
    event.key === 'Enter' &&
    (shortcut === 'shift-enter' ? event.shiftKey : !event.shiftKey) &&
    !event.isComposing &&
    !composing &&
    event.keyCode !== 229
  )
}

export function shouldFocusComposer(
  event: FocusKeyEvent,
  shortcut: 'space' | 'slash' | 'none',
  editableTarget: boolean,
  dialogOpen: boolean,
): boolean {
  const matches = shortcut === 'space' ? event.key === ' ' : shortcut === 'slash' && event.key === '/'
  return matches && !event.repeat && !event.metaKey && !event.ctrlKey && !event.altKey && !editableTarget && !dialogOpen
}
