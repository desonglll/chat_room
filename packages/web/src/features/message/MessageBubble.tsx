/**
 * `MessageBubble` — the single export TG-101's list renders every row through. It dispatches
 * on the row type: system rows become the centred service pill, local upload rows the
 * upload placeholder, and real messages the full bubble assembled here from the registry's
 * content, the decorations, the meta and the menu.
 */
import { PostCommentsEntry } from '../channel/comments/PostCommentsEntry'
import type { BroadcastMessage } from '@tg/core'
import type { MessageActions, MessageBubbleProps } from './types'
import { computeBubbleLayout } from './bubbleLayout'
import { messageContent } from './content/messageContent'
import { TextContent } from './content/builtinContents'
import { BubbleFrame } from './BubbleFrame'
import { ForwardHeader, ReactionRow, ReplyQuote, SenderName } from './BubbleParts'
import { HoverActions } from './HoverActions'
import { MessageMeta, MetaSpacer, metaParts, resolveDelivery } from './MessageMeta'
import { MessageRow } from './MessageRow'
import { buildMessageMenu, canReact } from './messageMenu'
import { noteOwnReaction } from './reactionIntent'
import { SystemMessage } from './SystemMessage'
import { UploadBubble } from './UploadBubble'

const NO_ACTIONS: MessageActions = {}

export function MessageBubble(props: MessageBubbleProps) {
  const { message } = props
  if (message.type === 'system') return <SystemMessage text={message.content} />
  if (message.type === 'upload') return <UploadBubble upload={message} ctx={props.ctx} />
  return <ChatBubble {...props} message={message} />
}

function ChatBubble({
  message,
  ctx,
  actions = NO_ACTIONS,
  viewerId,
  delivery: explicitDelivery,
  selectionMode = false,
  reserveAvatar,
}: MessageBubbleProps & { message: BroadcastMessage }) {
  const kind = messageContent.resolve(message)
  const Content = kind?.component ?? TextContent
  const recalled = message.recalled_at !== null
  const delivery = resolveDelivery(message, explicitDelivery)
  const facts = { delivered: delivery === 'sent' || delivery === 'read' }

  const showName = ctx.showSenderName && !ctx.isOutgoing
  const forward = recalled ? null : message.forwarded_from
  const reply = recalled ? null : message.reply_to
  const reactions = recalled ? [] : message.reactions
  const baseFrame = kind?.frame(message) ?? 'bubble'
  const layout = computeBubbleLayout({
    groupPosition: ctx.groupPosition,
    isOutgoing: ctx.isOutgoing,
    frame: baseFrame,
    metaPlacement: kind?.metaPlacement(message) ?? 'inline',
    hasSenderName: showName,
    hasForward: forward !== null,
    hasReply: reply !== null,
    hasReactions: reactions.length > 0,
  })

  const parts = metaParts(message, ctx.isOutgoing, delivery)
  const spacer = <MetaSpacer parts={parts} />
  const items = selectionMode ? [] : buildMessageMenu(message, actions, facts)
  const reactable = !selectionMode && canReact(message, actions, facts)
  const baseReact = actions.onReact
  const onReact =
    reactable && baseReact
      ? (emoji: string) => {
          const mine = reactions.some(
            (r) => r.emoji === emoji && viewerId !== undefined && r.user_ids.includes(viewerId),
          )
          if (!mine) noteOwnReaction(message.message_id, emoji)
          baseReact(emoji)
        }
      : undefined
  const media = kind?.leadingMedia ?? false

  return (
    <MessageRow
      ctx={ctx}
      selectionMode={selectionMode}
      reserveAvatar={reserveAvatar ?? ctx.showAvatar}
      avatar={{ label: message.sender, emoji: message.sender_avatar }}
      menuItems={items}
      onSelect={actions.onSelect}
      onReact={onReact}
      sentAt={message.timestamp}
    >
      <BubbleFrame
        layout={layout}
        flushTop={media && !showName && forward === null && reply === null}
        flushBottom={media && layout.metaMode === 'overlay'}
      >
        {showName ? <SenderName name={message.sender} /> : null}
        {forward === null ? null : <ForwardHeader from={forward} />}
        {reply === null ? null : (
          <ReplyQuote reply={reply} onJumpTo={actions.onJumpTo} onOpenSource={actions.onOpenReplySource} />
        )}
        <Content
          message={message}
          ctx={ctx}
          actions={actions}
          metaSpacer={layout.metaMode === 'inline' ? spacer : null}
        />
        {reactions.length === 0 ? null : (
          <ReactionRow
            messageId={message.message_id}
            reactions={reactions}
            viewerId={viewerId}
            onReact={onReact}
            metaSpacer={layout.metaMode === 'reactions' ? spacer : null}
          />
        )}
        <MessageMeta parts={parts} overlay={layout.metaMode === 'overlay'} dateTime={message.timestamp} />
      </BubbleFrame>
      {/* TG-203: a channel post's comments entry, under the bubble. */}
      {message.comments === undefined ? null : <PostCommentsEntry message={message} />}
      {selectionMode ? null : (
        <HoverActions
          items={items}
          onReply={facts.delivered && !recalled ? actions.onReply : undefined}
          onReact={onReact}
        />
      )}
    </MessageRow>
  )
}
