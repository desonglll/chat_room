// TG-104: reply → edit → cancel and every other bar transition, plus the store wiring.
import { describe, expect, test } from 'bun:test'
import { createComposerStore, selectComposerMode, selectDraft } from '../stores/composerStore'
import type { ComposerModeEvent, ComposerModeState } from './composerMode'
import { IDLE_COMPOSER_MODE, activeComposerBar, editIsDirty, transitionComposerMode } from './composerMode'

const run = (events: ComposerModeEvent[], from: ComposerModeState = IDLE_COMPOSER_MODE) =>
  events.reduce(transitionComposerMode, from)

describe('transitionComposerMode', () => {
  test('reply → edit → cancel restores the reply underneath', () => {
    const replying = run([{ type: 'reply', messageId: 'm1' }])
    expect(activeComposerBar(replying)).toEqual({ kind: 'reply', messageId: 'm1' })

    const editing = transitionComposerMode(replying, { type: 'edit', messageId: 'm2', text: 'old' })
    expect(activeComposerBar(editing)).toEqual({ kind: 'edit', messageId: 'm2' })
    expect(editing.replyToMessageId).toBe('m1')

    const cancelled = transitionComposerMode(editing, { type: 'cancel' })
    expect(activeComposerBar(cancelled)).toEqual({ kind: 'reply', messageId: 'm1' })
    expect(cancelled.edit).toBeNull()

    const idle = transitionComposerMode(cancelled, { type: 'cancel' })
    expect(activeComposerBar(idle)).toEqual({ kind: 'none' })
  })

  test('saving an edit closes only the edit layer', () => {
    const state = run([
      { type: 'reply', messageId: 'm1' },
      { type: 'edit', messageId: 'm2', text: 'old' },
      { type: 'editText', text: 'new' },
      { type: 'sent' },
    ])
    expect(state).toEqual({ replyToMessageId: 'm1', edit: null, forward: null })
  })

  test('sending a message consumes the reply and forward bars', () => {
    expect(run([{ type: 'reply', messageId: 'm1' }, { type: 'sent' }])).toEqual(IDLE_COMPOSER_MODE)
    expect(run([{ type: 'forward', messageIds: ['a'], fromChatId: 'c9' }, { type: 'sent' }])).toEqual(
      IDLE_COMPOSER_MODE,
    )
  })

  test('replying from inside an edit abandons the edit', () => {
    const state = run([
      { type: 'edit', messageId: 'm2', text: 'x' },
      { type: 'reply', messageId: 'm3' },
    ])
    expect(state).toEqual({ replyToMessageId: 'm3', edit: null, forward: null })
  })

  test('forward replaces a reply, and a reply replaces a forward', () => {
    const forwarding = run([
      { type: 'reply', messageId: 'm1' },
      { type: 'forward', messageIds: ['a', 'b'], fromChatId: 'c9' },
    ])
    expect(activeComposerBar(forwarding)).toEqual({ kind: 'forward', messageIds: ['a', 'b'], fromChatId: 'c9' })
    expect(forwarding.replyToMessageId).toBeNull()
    const replying = transitionComposerMode(forwarding, { type: 'reply', messageId: 'm5' })
    expect(replying).toEqual({ replyToMessageId: 'm5', edit: null, forward: null })
  })

  test('edit over a forward: cancel peels the edit, then the forward', () => {
    const state = run([
      { type: 'forward', messageIds: ['a'], fromChatId: 'c9' },
      { type: 'edit', messageId: 'm2', text: 'x' },
    ])
    const once = transitionComposerMode(state, { type: 'cancel' })
    expect(activeComposerBar(once).kind).toBe('forward')
    expect(activeComposerBar(transitionComposerMode(once, { type: 'cancel' })).kind).toBe('none')
  })

  test('meaningless events keep the same reference', () => {
    expect(transitionComposerMode(IDLE_COMPOSER_MODE, { type: 'cancel' })).toBe(IDLE_COMPOSER_MODE)
    expect(transitionComposerMode(IDLE_COMPOSER_MODE, { type: 'sent' })).toBe(IDLE_COMPOSER_MODE)
    expect(transitionComposerMode(IDLE_COMPOSER_MODE, { type: 'editText', text: 'x' })).toBe(IDLE_COMPOSER_MODE)
    expect(transitionComposerMode(IDLE_COMPOSER_MODE, { type: 'reply', messageId: '' })).toBe(IDLE_COMPOSER_MODE)
    expect(transitionComposerMode(IDLE_COMPOSER_MODE, { type: 'forward', messageIds: [], fromChatId: 'c' })).toBe(
      IDLE_COMPOSER_MODE,
    )
  })

  test('editIsDirty ignores whitespace-only changes and refuses an emptied edit', () => {
    expect(editIsDirty({ messageId: 'm', originalText: 'hi', text: 'hi ' })).toBe(false)
    expect(editIsDirty({ messageId: 'm', originalText: 'hi', text: 'hey' })).toBe(true)
    expect(editIsDirty({ messageId: 'm', originalText: 'hi', text: '  ' })).toBe(false)
  })
})

describe('composerStore.dispatchMode', () => {
  test('the reply target lives in the synced draft; edit text never touches the draft', () => {
    const store = createComposerStore()
    store.getState().setDraftText('c1', 'draft words')
    store.getState().dispatchMode('c1', { type: 'reply', messageId: 'm1' })
    store.getState().dispatchMode('c1', { type: 'edit', messageId: 'm2', text: 'original' })
    store.getState().dispatchMode('c1', { type: 'editText', text: 'changed' })
    expect(selectDraft('c1')(store.getState())).toMatchObject({ text: 'draft words', replyToMessageId: 'm1' })
    expect(store.getState().editing.c1?.text).toBe('changed')

    store.getState().dispatchMode('c1', { type: 'cancel' })
    expect(selectComposerMode('c1')(store.getState())).toEqual({ replyToMessageId: 'm1', edit: null, forward: null })
    expect(selectDraft('c1')(store.getState()).text).toBe('draft words')
  })

  test('chats are independent', () => {
    const store = createComposerStore()
    store.getState().dispatchMode('c1', { type: 'forward', messageIds: ['x'], fromChatId: 'c0' })
    expect(selectComposerMode('c2')(store.getState())).toEqual(IDLE_COMPOSER_MODE)
    expect(store.getState().forwarding.c1?.messageIds).toEqual(['x'])
  })
})
