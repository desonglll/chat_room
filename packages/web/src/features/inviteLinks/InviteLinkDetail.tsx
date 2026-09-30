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
          {when ? `${person.status === 'pending' ? '申请于' : '加入于'} ${formatWhen(when)}` : `@${person.username}`}
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
            ? `由 ${link.creator_name} 创建于 ${formatWhen(link.created_at)}`
            : `创建于 ${formatWhen(link.created_at)}`}
        </p>
      </section>
      <LinkShareBox token={link.token} live={link.state === 'active'} origin={props.origin} />
      {canChange ? (
        <div className="tg-invite__actions" data-stack="">
          {live && !link.is_primary ? (
            <Button variant="tonal" onClick={props.onEdit} disabled={busy}>
              编辑链接
            </Button>
          ) : null}
          {live && !confirming ? (
            <Button variant="danger" onClick={() => setConfirming(true)} disabled={busy}>
              撤销链接
            </Button>
          ) : null}
          {live && confirming ? (
            <div className="tg-invite__confirm" role="group" aria-label="确认撤销">
              <span>{link.is_primary ? '撤销后旧链接立即失效，并生成新的主链接。' : '撤销后该链接立即失效。'}</span>
              <Button variant="danger" onClick={props.onRevoke} loading={busy}>
                确认撤销
              </Button>
              <Button variant="text" onClick={() => setConfirming(false)}>
                取消
              </Button>
            </div>
          ) : null}
          {!live ? (
            <Button variant="danger" onClick={props.onDelete} disabled={busy}>
              删除链接
            </Button>
          ) : null}
        </div>
      ) : null}
      {!people ? (
        <Spinner label="正在加载" />
      ) : (
        <>
          {people.pending.length > 0 ? (
            <section aria-label="加入申请">
              <h4 className="tg-invite__section-title tg-invite__list-title">加入申请 · {people.pending.length}</h4>
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
                          通过
                        </Button>
                        <Button
                          size="sm"
                          variant="text"
                          disabled={busy}
                          onClick={() => void props.onSettle(person.user_id, false)}
                        >
                          拒绝
                        </Button>
                      </span>
                    ) : null}
                  </PersonRow>
                ))}
              </ul>
            </section>
          ) : null}
          <section aria-label="已加入">
            <h4 className="tg-invite__section-title tg-invite__list-title">通过此链接加入 · {people.joined.length}</h4>
            {people.joined.length === 0 ? (
              <p className="tg-invite__note">还没有人通过此链接加入。</p>
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
