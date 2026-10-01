/** The presentational half of the invite landing page, rendered from a plain state value. */
import type { InvitePreview, InviteRefusal } from '@tg/core'
import { Avatar, Button, Spinner } from '@tg/ui'
import { REFUSAL_COPY } from './inviteLinksModel'
import { t } from '../../i18n/index'

export type JoinCardState =
  | { kind: 'loading' }
  | { kind: 'preview'; preview: InvitePreview; busy: boolean }
  | { kind: 'refused'; reason: InviteRefusal }

const TYPE_LABEL: Record<InvitePreview['chat_type'], string> = {
  get private() {
    return t('w.inviteLinks.3adfdb')
  },
  get group() {
    return t('w.inviteLinks.4260ca')
  },
  get supergroup() {
    return t('w.inviteLinks.a06237')
  },
  get channel() {
    return t('w.inviteLinks.b76dfd')
  },
}

function actionLabel(preview: InvitePreview): string {
  if (preview.membership_status === 'active') return t('w.inviteLinks.745cd2')
  if (preview.requires_approval) return t('w.inviteLinks.527fa6')
  return preview.chat_type === 'channel' ? t('w.inviteLinks.9cdcda') : t('w.inviteLinks.9542b3')
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
    content = <Spinner label={t('w.inviteLinks.f3cfa9')} />
  } else if (state.kind === 'refused') {
    content = (
      <>
        <p className="tg-join__title">{REFUSAL_COPY[state.reason]}</p>
        <p className="tg-invite__meta">{t('w.inviteLinks.2b9919')}</p>
        <Button variant="tonal" onClick={onDismiss}>
          {t('w.inviteLinks.11d024')}
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
          {TYPE_LABEL[preview.chat_type]} ·{' '}
          {preview.chat_type === 'channel'
            ? t('w.channel.2b75a3', preview.member_count)
            : t('w.chatInfo.0ece20', preview.member_count)}
        </p>
        {preview.description ? <p className="tg-join__description">{preview.description}</p> : null}
        {pending ? (
          <p className="tg-join__pending" role="status">
            {t('w.inviteLinks.bd6821')}
          </p>
        ) : (
          <>
            {preview.requires_approval && preview.membership_status !== 'active' ? (
              <p className="tg-invite__meta">{t('w.inviteLinks.d452a9')}</p>
            ) : null}
            <Button variant="filled" loading={busy} disabled={busy} onClick={() => onJoin(preview)}>
              {actionLabel(preview)}
            </Button>
          </>
        )}
        <Button variant="text" onClick={onDismiss}>
          {t('w.inviteLinks.4d0b46')}
        </Button>
      </>
    )
  }
  return (
    <div className="tg-join">
      <section className="tg-join__card" aria-label={t('w.inviteLinks.8a8f47')} aria-busy={state.kind === 'loading'}>
        {content}
      </section>
    </div>
  )
}
