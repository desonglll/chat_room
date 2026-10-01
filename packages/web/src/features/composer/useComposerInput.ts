/**
 * Everything the textarea itself does: caret/selection tracking, text splicing with
 * caret restore, @mention state, and the keydown router (mention navigation → format
 * shortcuts → quick calculator → Escape → ↑ edits last → Enter sends). Pure decisions
 * come from `@tg/core` (`findMentionQuery`, `formatShortcut`, `applyFormat`,
 * `shouldCalculateExpression`); this hook only wires them to DOM events.
 */
import { replyStep } from '../shortcuts/keymap'
import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from 'react'
import type { ChatMember, FormatKind, SendShortcut, TextSelection } from '@tg/core'
import {
  applyFormat,
  applyMention,
  evaluateArithmeticExpression,
  findMentionQuery,
  formatShortcut,
  matchMentionCandidates,
  shouldCalculateExpression,
} from '@tg/core'
import type { ComposerController } from './composerController'
import type { PickedCustomEmoji } from '../customEmoji/CustomEmojiGrid'

export interface ComposerInputOptions {
  text: string
  controller: ComposerController
  members: readonly ChatMember[]
  currentUserId: string
  sendShortcut: SendShortcut
  onSubmit(): void
  /** ↑ in an empty input: Telegram edits your last message. */
  onEditLast(): boolean
  /** TG-606: Ctrl/⌘+↑/↓ — reply to an older (−1) or newer (+1) message; false = not handled. */
  onReplyStep?(step: -1 | 1): boolean
}

export interface ComposerInputHandle {
  textareaRef: RefObject<HTMLTextAreaElement | null>
  selection: TextSelection
  mentionCandidates: readonly ChatMember[]
  mentionIndex: number
  setMentionIndex(index: number): void
  pickMention(member: ChatMember): void
  format(kind: FormatKind, url?: string): void
  insert(value: string): void
  /** TG-1206: a custom emoji from the panel — fallback text plus its entity in the draft. */
  insertCustom(emoji: PickedCustomEmoji): void
  calcError: string
  syncSelection(): void
  onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void
  onChange(text: string): void
}

export function useComposerInput(options: ComposerInputOptions): ComposerInputHandle {
  const { text, controller } = options
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)
  const pendingCaret = useRef<TextSelection | null>(null)
  const [selection, setSelection] = useState<TextSelection>({ start: text.length, end: text.length })
  const [mentionIndex, setMentionIndex] = useState(0)
  const [dismissedMentionAt, setDismissedMentionAt] = useState(-1)
  const [calcError, setCalcError] = useState('')

  const mention = selection.start === selection.end ? findMentionQuery(text, selection.end) : null
  const mentionCandidates = useMemo(
    () =>
      mention && mention.start !== dismissedMentionAt
        ? matchMentionCandidates(options.members, mention.query, { excludeUserId: options.currentUserId })
        : [],
    [mention?.start, mention?.query, dismissedMentionAt, options.members, options.currentUserId],
  )
  const activeMention = Math.min(mentionIndex, Math.max(0, mentionCandidates.length - 1))

  // Restore the caret after a programmatic edit once React has written the new value.
  useLayoutEffect(() => {
    const target = pendingCaret.current
    const element = textareaRef.current
    if (!target || !element) return
    pendingCaret.current = null
    element.focus()
    element.setSelectionRange(target.start, target.end)
    setSelection(target)
  }, [text])

  function replace(next: string, caret: TextSelection) {
    pendingCaret.current = caret
    setCalcError('')
    controller.input(next)
  }

  function syncSelection() {
    const element = textareaRef.current
    if (!element) return
    const next = { start: element.selectionStart, end: element.selectionEnd }
    setSelection((current) => (current.start === next.start && current.end === next.end ? current : next))
  }

  function currentSelection(): TextSelection {
    const element = textareaRef.current
    return element ? { start: element.selectionStart, end: element.selectionEnd } : selection
  }

  function pickMention(member: ChatMember) {
    if (!mention) return
    const result = applyMention(text, mention, member.username)
    setMentionIndex(0)
    replace(result.text, { start: result.caret, end: result.caret })
  }

  function format(kind: FormatKind, url?: string) {
    const result = applyFormat(text, currentSelection(), kind, url)
    replace(result.text, result.selection)
  }

  function insert(value: string) {
    const { start, end } = currentSelection()
    const caret = start + value.length
    replace(text.slice(0, start) + value + text.slice(end), { start: caret, end: caret })
  }

  function insertCustom(emoji: PickedCustomEmoji) {
    const result = controller.insertCustomEmoji(currentSelection(), emoji)
    if (!result) {
      insert(emoji.emoji)
      return
    }
    pendingCaret.current = { start: result.caret, end: result.caret }
    setCalcError('')
  }

  function handleMentionKeys(event: KeyboardEvent<HTMLTextAreaElement>): boolean {
    if (mentionCandidates.length === 0 || !mention) return false
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      const step = event.key === 'ArrowDown' ? 1 : -1
      setMentionIndex((activeMention + step + mentionCandidates.length) % mentionCandidates.length)
      return true
    }
    if ((event.key === 'Enter' && !event.shiftKey) || event.key === 'Tab') {
      pickMention(mentionCandidates[activeMention]!)
      return true
    }
    if (event.key === 'Escape') {
      setDismissedMentionAt(mention.start)
      return true
    }
    return false
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    const native = event.nativeEvent
    const composing = native.isComposing || native.keyCode === 229
    if (composing) return
    if (handleMentionKeys(event)) {
      event.preventDefault()
      return
    }
    const formatKind = formatShortcut(event)
    if (formatKind) {
      event.preventDefault()
      if (formatKind === 'link') format('link', '')
      else format(formatKind)
      return
    }
    const calculatorKey = {
      key: event.key,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      metaKey: event.metaKey,
      ctrlKey: event.ctrlKey,
      isComposing: native.isComposing,
      keyCode: native.keyCode,
    }
    if (shouldCalculateExpression(calculatorKey, composing)) {
      event.preventDefault()
      const result = evaluateArithmeticExpression(text)
      if (result.ok) replace(result.value, { start: result.value.length, end: result.value.length })
      else setCalcError(result.error)
      return
    }
    if (event.key === 'Escape') {
      if (controller.cancel()) event.preventDefault()
      return
    }
    const step = replyStep({ ...calculatorKey, editable: true })
    if (step !== null) {
      if (options.onReplyStep?.(step)) event.preventDefault()
      return
    }
    if (event.key === 'ArrowUp' && text === '' && !event.shiftKey && !event.altKey) {
      if (options.onEditLast()) event.preventDefault()
      return
    }
    if (event.key !== 'Enter' || event.altKey || event.ctrlKey || event.metaKey) return
    const sends = options.sendShortcut === 'shift-enter' ? event.shiftKey : !event.shiftKey
    if (sends) {
      event.preventDefault()
      options.onSubmit()
    }
  }

  return {
    textareaRef,
    selection,
    mentionCandidates,
    mentionIndex: activeMention,
    setMentionIndex,
    pickMention,
    format,
    insert,
    insertCustom,
    calcError,
    syncSelection,
    onKeyDown,
    onChange(next) {
      setCalcError('')
      setDismissedMentionAt(-1)
      setMentionIndex(0)
      controller.input(next)
    },
  }
}
