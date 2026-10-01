/**
 * TG-104 — the Telegram input area. Layout, left to right: emoji, auto-growing textarea
 * (with the reply/edit/forward bar above it, @mention list and selection toolbar
 * floating over it), paperclip, and the send ↔ voice button. Behaviour lives in
 * `composerController.ts` (stores, draft sync, socket) and `useComposerInput.ts`
 * (keyboard, caret); pasted/dropped/picked files go to `usePendingBatch`. TG-404: the send
 * button's long-press menu (`SendMenu`) and the scheduled-messages entry (`ScheduledEntry`).
 */
import {
  lazy,
  Suspense,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
} from 'react'
import type { ChatMember, Sticker } from '@tg/core'
import {
  activeComposerBar,
  composerStore,
  evaluateArithmeticExpression,
  joinClauses,
  labelled,
  messageStore,
  settingsStore,
} from '@tg/core'
import { IconButton, Popover } from '@tg/ui'
import { useStore } from 'zustand/react'
import { AttachMenu } from './AttachMenu'
import { nextReplyTarget } from '../shortcuts/keymap'
import { ComposerBar } from './ComposerBar'
import { ComposerLinkPreview } from '../linkPreview/ComposerLinkPreview'
import type { ComposerSessionApi } from './composerController'
import { DropZone } from './DropZone'
import { ComposerEmojiTab } from './ComposerEmojiTab'
import { FormatToolbar } from './FormatToolbar'
import { CheckGlyph, MicGlyph, SendGlyph, SmileGlyph } from './icons'
import { MentionPopup, mentionOptionId } from './MentionPopup'
import { PendingDialog } from './PendingDialog'
import { ScheduledEntry } from './ScheduledEntry'
import { SendMenu } from './SendMenu'
import { useComposerController } from './useComposerController'
import { useComposerInput } from './useComposerInput'
import { usePendingBatch } from './usePendingBatch'
// Direct module imports, not the `../sticker` barrel: the barrel also re-exports the
// renderer, which would pull it into the entry chunk.
import { LazyMediaPanel } from '../sticker/LazyMediaPanel'
import { stickerLibrary } from '../sticker/stickerLibrary'
import { StickerSuggestions } from '../sticker/suggest/StickerSuggestions'
import { t } from '../../i18n/index'
import { useForwardNotice } from './forwardNotice'

export interface ComposerProps {
  chatId: string
  currentUserId: string
  /** The chat's members, for @mention autocomplete. */
  members: readonly ChatMember[]
  session: ComposerSessionApi
  /** False while the chat socket is not online: typing still works, sending is disabled. */
  canSend?: boolean
}

/** A draft that is only arithmetic gets a «= 42 · Alt+Enter» hint (quick calculator). */
const ARITHMETIC = /^[\d\s.+\-*/×÷()（）]*\d[\d\s.+\-*/×÷()（）]*[+\-*/×÷][\d\s.+\-*/×÷()（）]*\d[\s)）]*$/

/** TG-606: messages a keyboard reply can target, oldest first. */
function replyableIds(chatId: string): string[] {
  return (messageStore.getState().timelines[chatId]?.messages ?? []).flatMap((message) =>
    message.type === 'broadcast' && !message.recalled_at && !message.message_id.startsWith('pending:')
      ? [message.message_id]
      : [],
  )
}

function lastOwnMessageId(chatId: string, userId: string): string | null {
  const list = messageStore.getState().timelines[chatId]?.messages ?? []
  for (let index = list.length - 1; index >= 0; index -= 1) {
    const message = list[index]!
    if (message.type === 'broadcast' && message.sender_id === userId && !message.recalled_at && message.content) {
      return message.message_id
    }
  }
  return null
}

// TG-1003: the voice / round-video recorder (MediaRecorder, waveform, viewfinder) is not needed
// for first paint; a same-looking disabled mic holds its place until the chunk arrives.
const RecordModeButton = lazy(() =>
  import('../videoNote/RecordModeButton').then((module) => ({ default: module.RecordModeButton })),
)

export function Composer({ chatId, currentUserId, members, session, canSend = true }: ComposerProps) {
  const { sendMessage, setDraftText, sendFrame } = session
  const stableSession = useMemo(
    () => ({ sendMessage, setDraftText, sendFrame }),
    [sendMessage, setDraftText, sendFrame],
  )
  const controller = useComposerController(chatId, stableSession)
  const forwardNotice = useForwardNotice(chatId)
  const draftText = useStore(composerStore, (state) => state.drafts[chatId]?.text ?? '')
  const replyTo = useStore(composerStore, (state) => state.drafts[chatId]?.replyToMessageId ?? null)
  const edit = useStore(composerStore, (state) => state.editing[chatId] ?? null)
  const forward = useStore(composerStore, (state) => state.forwarding[chatId] ?? null)
  const sendShortcut = useStore(settingsStore, (state) => state.sendShortcut)
  const bar = activeComposerBar({ replyToMessageId: replyTo, edit, forward })
  const text = edit ? edit.text : draftText

  const rootRef = useRef<HTMLElement>(null)
  const emojiRef = useRef<HTMLButtonElement>(null)
  const [emojiOpen, setEmojiOpen] = useState(false)
  const mentionListId = useId()
  const pending = usePendingBatch(chatId, controller, replyTo)

  const submit = () => {
    if (!canSend) return
    controller.submit()
  }

  const input = useComposerInput({
    text,
    controller,
    members,
    currentUserId,
    sendShortcut,
    onSubmit: submit,
    onEditLast: () => {
      const id = lastOwnMessageId(chatId, currentUserId)
      return id ? controller.edit(id) : false
    },
    onReplyStep: (step) => {
      const current = composerStore.getState().drafts[chatId]?.replyToMessageId ?? null
      const next = nextReplyTarget(replyableIds(chatId), current, step)
      if (next) controller.reply(next)
      else if (current) controller.cancel()
      return next !== null || current !== null
    },
  })

  // Auto-height: grow with the content up to the CSS max-height, then scroll.
  useLayoutEffect(() => {
    const element = input.textareaRef.current
    if (!element) return
    element.style.height = 'auto'
    element.style.height = `${element.scrollHeight}px`
  }, [text, input.textareaRef])

  // Entering edit mode (from a message menu or ↑) focuses the input with the caret at the end.
  useLayoutEffect(() => {
    const element = input.textareaRef.current
    if (!edit || !element) return
    element.focus()
    element.setSelectionRange(element.value.length, element.value.length)
    // Only when the edited message changes, not on every edit keystroke.
  }, [edit?.messageId, input.textareaRef])

  // A reply target set from outside (a message's «回复» menu writes the store directly)
  // still has to reach the cloud draft, so every change is pushed through the session.
  const syncedReply = useRef({ chatId, replyTo })
  useEffect(() => {
    const previous = syncedReply.current
    syncedReply.current = { chatId, replyTo }
    // A chat switch is not a reply change: the new chat's value came from its own draft.
    if (previous.chatId !== chatId || previous.replyTo === replyTo) return
    setDraftText(composerStore.getState().drafts[chatId]?.text ?? '')
    if (replyTo) input.textareaRef.current?.focus()
  }, [chatId, replyTo, setDraftText, input.textareaRef])

  const addFiles = (files: File[], asFiles: boolean) => {
    if (asFiles && pending.batch.items.length === 0) pending.setSendAsFiles(true)
    pending.add(files)
  }

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.clipboardData.files)
    if (files.length === 0) return
    event.preventDefault()
    addFiles(files, false)
  }

  // TG-303: a sticker goes out over HTTP (not the socket) and consumes the reply bar.
  const sendSticker = (sticker: Sticker) => {
    if (!canSend) return
    void stickerLibrary().send({ chatId, sticker, replyTo })
    controller.consumeReply()
  }

  const hasPayload = text.trim() !== '' || bar.kind === 'forward'
  const showSend = hasPayload || bar.kind === 'edit'
  const calc = !edit && ARITHMETIC.test(text) ? evaluateArithmeticExpression(text) : null
  const selecting = input.selection.start !== input.selection.end
  const mentionOpen = input.mentionCandidates.length > 0

  return (
    <footer className="tg-compose" ref={rootRef} data-mode={bar.kind}>
      <div className="tg-compose__field">
        <ComposerBar chatId={chatId} bar={bar} onCancel={() => controller.cancel()} />
        {/* TG-408: the draft's link card, dismissible. */}
        {bar.kind !== 'edit' ? <ComposerLinkPreview chatId={chatId} text={text} /> : null}
        {mentionOpen ? (
          <MentionPopup
            id={mentionListId}
            candidates={input.mentionCandidates}
            activeIndex={input.mentionIndex}
            onPick={input.pickMention}
            onHover={input.setMentionIndex}
          />
        ) : null}
        {selecting && !mentionOpen ? <FormatToolbar onFormat={input.format} /> : null}
        {edit || mentionOpen ? null : (
          <StickerSuggestions
            draft={text}
            disabled={!canSend}
            onPick={(sticker) => {
              sendSticker(sticker)
              controller.input('')
            }}
          />
        )}
        <div className="tg-compose__row">
          <IconButton
            ref={emojiRef}
            label={t('w.composer.fecd66')}
            className="tg-compose__tool"
            aria-expanded={emojiOpen}
            onClick={() => setEmojiOpen((open) => !open)}
          >
            <SmileGlyph />
          </IconButton>
          <textarea
            ref={input.textareaRef}
            className="tg-compose__input"
            value={text}
            rows={1}
            placeholder={edit ? t('w.composer.4c92ed') : t('w.composer.ae4564')}
            aria-label={t('w.composer.acbc5a')}
            role={mentionOpen ? 'combobox' : undefined}
            aria-expanded={mentionOpen ? true : undefined}
            aria-controls={mentionOpen ? mentionListId : undefined}
            aria-activedescendant={mentionOpen ? mentionOptionId(mentionListId, input.mentionIndex) : undefined}
            onChange={(event) => {
              input.onChange(event.target.value)
              input.syncSelection()
            }}
            onSelect={input.syncSelection}
            onKeyDown={input.onKeyDown}
            onPaste={onPaste}
          />
          {calc?.ok ? (
            <span className="tg-compose__calc" aria-live="polite">
              = {calc.value}
              <kbd>Alt+Enter</kbd>
            </span>
          ) : null}
          <ScheduledEntry chatId={chatId} />
          {bar.kind !== 'edit' ? <AttachMenu chatId={chatId} onFiles={addFiles} /> : null}
        </div>
        {forwardNotice ? (
          <p className="tg-compose__notice" role="alert">
            {forwardNotice}
          </p>
        ) : null}
        {input.calcError ? (
          <p className="tg-compose__notice" role="alert">
            {input.calcError}
          </p>
        ) : null}
        {pending.rejected.length > 0 && pending.batch.items.length === 0 ? (
          <p className="tg-compose__notice" role="alert">
            {joinClauses(pending.rejected.map((file) => labelled(file.name, file.reason)))}
          </p>
        ) : null}
      </div>
      {showSend ? (
        <SendMenu chatId={chatId} controller={controller} enabled={canSend && bar.kind !== 'edit'}>
          <IconButton
            label={bar.kind === 'edit' ? t('w.composer.fadf24') : t('w.composer.1214d6')}
            variant="filled"
            size="lg"
            className="tg-compose__send"
            data-kind={bar.kind === 'edit' ? 'save' : 'send'}
            disabled={!canSend}
            onClick={submit}
          >
            {bar.kind === 'edit' ? <CheckGlyph /> : <SendGlyph />}
          </IconButton>
        </SendMenu>
      ) : (
        // TG-401/TG-402: tap toggles mic ↔ camera; hold records, slide cancels, slide up locks.
        <Suspense
          fallback={
            <IconButton
              label={t('w.composer.recorderLoading')}
              variant="filled"
              size="lg"
              className="tg-compose__send"
              disabled
            >
              <MicGlyph />
            </IconButton>
          }
        >
          <RecordModeButton
            chatId={chatId}
            replyTo={replyTo}
            canSend={canSend}
            sendFrame={sendFrame}
            onSent={() => controller.consumeReply()}
            micGlyph={<MicGlyph />}
            sendGlyph={<SendGlyph />}
          />
        </Suspense>
      )}
      <Popover
        open={emojiOpen}
        onClose={() => setEmojiOpen(false)}
        anchor={emojiRef}
        insideRefs={[emojiRef]}
        placement="top-start"
        offset={12}
        surface="panel"
        focus="none"
        role="dialog"
        aria-label={t('w.composer.fecd66')}
        className="tg-compose__emoji-popover"
      >
        <LazyMediaPanel
          chatId={chatId}
          emoji={<ComposerEmojiTab onPick={input.insert} onPickCustom={input.insertCustom} />}
          canSend={canSend}
          onSendSticker={(sticker) => {
            sendSticker(sticker)
            setEmojiOpen(false)
          }}
          onClose={() => setEmojiOpen(false)}
        />
      </Popover>
      <DropZone anchorRef={rootRef} onFiles={addFiles} />
      <PendingDialog pending={pending} />
    </footer>
  )
}
