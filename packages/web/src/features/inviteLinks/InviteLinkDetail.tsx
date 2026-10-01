/**
 * One link: the URL with copy / share, its facts, the actions the viewer may take, the join
 * requests that arrived through it (approve / decline with the existing member action), and
 * the members it admitted.
 */
import { useEffect, useState } from 'react'
import type { InviteLink, InviteLinkMember } from '@tg/core'
import { Avatar, Button, Spinner } from '@tg/ui'
import { formatWhen, linkName, linkSummary } from './inviteLinksModel'
import { LinkShareBox } from './LinkShareBox'
import type { LinkPeople } from './useInviteLinks'
import { t } from '../../i18n/index'

export interface InviteLinkDetailProps {
  link: InviteLink
  canChange: boolean
  canReview: boolean
  busy: boolean
  loadPeople(): Promise<LinkPeople>
  onEdit(): void
  onRevoke(): void
  onDelete(): void
  onSettle(userId: string, approve: boolean): Promise<unknown>
  /** Test/screenshot seed. */
  initialPeople?: LinkPeople | undefined
  origin?: string | undefined
}

function personName(person: InviteLinkMember): string {
  return person.display_name || person.username
}

function PersonRow({ person, children }: { person: InviteLinkMember; children?: React.ReactNode }) {
  const when = person.status === 'pending' ? person.requested_at : person.joined_at
  return (
    <li className="tg-invite__person">
      <Avatar label={personName(person)} initials={person.avatar_emoji || undefined} size="sm" />
      <span className="tg-invite__person-text">
        <span className="tg-invite__person-name">{personName(person)}</span>
        <span className="tg-invite__meta">
          {when
            ? `${person.status === 'pending' ? t('w.inviteLinks.2af85c') : t('w.inviteLinks.c5d3fe')} ${formatWhen(when)}`
            : `@${person.username}`}
        </span>
      </span>
      {children}
    </li>
  )
}

export function InviteLinkDetail(props: InviteLinkDetailProps) {
  const { link, canChange, canReview, busy, loadPeople, initialPeople } = props
  const [people, setPeople] = useState<LinkPeople | null>(initialPeople ?? null)
  const [confirming, setConfirming] = useState(false)
  const live = link.state !== 'revoked'

  useEffect(() => {
    if (initialPeople) return
    let alive = true
    loadPeople().then(
      (answer) => alive && setPeople(answer),
      () => alive && setPeople({ joined: [], pending: [] }),
    )
    return () => {
      alive = false
    }
  }, [loadPeople, initialPeople, link.usage_count, link.pending_count])

  return (
    <div className="tg-invite__detail">
      <section className="tg-invite__section">
        <h3 className="tg-invite__title">{linkName(link)}</h3>
        <p className="tg-invite__meta">{linkSummary(link)}</p>
        <p className="tg-invite__meta">
          {link.creator_name
            ? t('w.inviteLinks.897fae', link.creator_name, formatWhen(link.created_at))
            : t('w.inviteLinks.be5695', formatWhen(link.created_at))}
        </p>
      </section>
      <LinkShareBox token={link.token} live={link.state === 'active'} origin={props.origin} />
      {canChange ? (
        <div className="tg-invite__actions" data-stack="">
          {live && !link.is_primary ? (
            <Button variant="tonal" onClick={props.onEdit} disabled={busy}>
              {t('w.inviteLinks.48513c')}
            </Button>
          ) : null}
          {live && !confirming ? (
            <Button variant="danger" onClick={() => setConfirming(true)} disabled={busy}>
              {t('w.inviteLinks.4e4919')}
            </Button>
          ) : null}
          {live && confirming ? (
            <div className="tg-invite__confirm" role="group" aria-label={t('w.inviteLinks.f7e81a')}>
              <span>{link.is_primary ? t('w.inviteLinks.ea10df') : t('w.inviteLinks.fe46dc')}</span>
              <Button variant="danger" onClick={props.onRevoke} loading={busy}>
                {t('w.inviteLinks.f7e81a')}
              </Button>
              <Button variant="text" onClick={() => setConfirming(false)}>
                {t('w.inviteLinks.4d0b46')}
              </Button>
            </div>
          ) : null}
          {!live ? (
            <Button variant="danger" onClick={props.onDelete} disabled={busy}>
              {t('w.inviteLinks.5f8cb7')}
            </Button>
          ) : null}
        </div>
      ) : null}
      {!people ? (
        <Spinner label={t('w.inviteLinks.3667cb')} />
      ) : (
        <>
          {people.pending.length > 0 ? (
            <section aria-label={t('w.inviteLinks.ff7bac')}>
              <h4 className="tg-invite__section-title tg-invite__list-title">
                {t('w.inviteLinks.7ac0f4')} {people.pending.length}
              </h4>
              <ul className="tg-invite__people">
                {people.pending.map((person) => (
                  <PersonRow key={person.user_id} person={person}>
                    {canReview ? (
                      <span className="tg-invite__row-actions">
                        <Button
                          size="sm"
                          variant="filled"
                          disabled={busy}
                          onClick={() => void props.onSettle(person.user_id, true)}
                        >
                          {t('w.inviteLinks.dcc423')}
                        </Button>
                        <Button
                          size="sm"
                          variant="text"
                          disabled={busy}
                          onClick={() => void props.onSettle(person.user_id, false)}
                        >
                          {t('w.inviteLinks.03e210')}
                        </Button>
                      </span>
                    ) : null}
                  </PersonRow>
                ))}
              </ul>
            </section>
          ) : null}
          <section aria-label={t('w.inviteLinks.7def19')}>
            <h4 className="tg-invite__section-title tg-invite__list-title">
              {t('w.inviteLinks.0db793')} {people.joined.length}
            </h4>
            {people.joined.length === 0 ? (
              <p className="tg-invite__note">{t('w.inviteLinks.d5fea3')}</p>
            ) : (
              <ul className="tg-invite__people">
                {people.joined.map((person) => (
                  <PersonRow key={person.user_id} person={person} />
                ))}
              </ul>
            )}
          </section>
        </>
      )}
    </div>
  )
}
