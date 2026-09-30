/** The presentational half of the invite landing page, rendered from a plain state value. */
import type { InvitePreview, InviteRefusal } from '@tg/core'
import { Avatar, Button, Spinner } from '@tg/ui'
import { REFUSAL_COPY } from './inviteLinksModel'

export type JoinCardState =
  | { kind: 'loading' }
  | { kind: 'preview'; preview: InvitePreview; busy: boolean }
  | { kind: 'refused'; reason: InviteRefusal }

const TYPE_LABEL: Record<InvitePreview['chat_type'], string> = {
  private: '私聊',
  group: '群组',
  supergroup: '超级群',
  channel: '频道',
}

function actionLabel(preview: InvitePreview): string {
  if (preview.membership_status === 'active') return '打开聊天'
  if (preview.requires_approval) return '申请加入'
  return preview.chat_type === 'channel' ? '加入频道' : '加入群组'
}

export function JoinChatCard({
  state,
  onJoin,
  onDismiss,
}: {
  state: JoinCardState
  onJoin(preview: InvitePreview): void
  onDismiss(): void
}) {
  let content
  if (state.kind === 'loading') {
    content = <Spinner label="正在打开邀请链接" />
  } else if (state.kind === 'refused') {
    content = (
      <>
        <p className="tg-join__title">{REFUSAL_COPY[state.reason]}</p>
        <p className="tg-invite__meta">请向群管理员索取新的邀请链接。</p>
        <Button variant="tonal" onClick={onDismiss}>
          返回
        </Button>
      </>
    )
  } else {
    const { preview, busy } = state
    const pending = preview.membership_status === 'pending'
    content = (
      <>
        <Avatar label={preview.title} initials={preview.avatar_emoji || undefined} size="xl" />
        <p className="tg-join__title">{preview.title}</p>
        <p className="tg-invite__meta">
          {TYPE_LABEL[preview.chat_type]} · {preview.member_count} 位成员
        </p>
        {preview.description ? <p className="tg-join__description">{preview.description}</p> : null}
        {pending ? (
          <p className="tg-join__pending" role="status">
            已发送加入申请，等待管理员审核。
          </p>
        ) : (
          <>
            {preview.requires_approval && preview.membership_status !== 'active' ? (
              <p className="tg-invite__meta">管理员审核通过后你才会加入。</p>
            ) : null}
            <Button variant="filled" loading={busy} disabled={busy} onClick={() => onJoin(preview)}>
              {actionLabel(preview)}
            </Button>
          </>
        )}
        <Button variant="text" onClick={onDismiss}>
          取消
        </Button>
      </>
    )
  }
  return (
    <div className="tg-join">
      <section className="tg-join__card" aria-label="邀请链接" aria-busy={state.kind === 'loading'}>
        {content}
      </section>
    </div>
  )
}
