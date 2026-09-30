/**
 * The invite-links page (TG-205): Telegram's "邀请链接" — the primary link with copy / share /
 * replace, the additional links with their usage and expiry, the revoked ones, and one level
 * down a link's detail (stats, joined list, pending requests) and the create / edit form.
 *
 * Navigation is a small in-panel stack, like `ChatAdminPanel`; `InviteLinksEntry` hosts it in a
 * right sheet.
 */
import { useCallback, useState, type ReactNode } from 'react'
import type { InviteLink } from '@tg/core'
import { Button, IconButton, ScrollArea, Spinner } from '@tg/ui'
import type { InviteLinksService } from './inviteLinksApi'
import { InviteLinkDetail } from './InviteLinkDetail'
import { InviteLinkEditor } from './InviteLinkEditor'
import { canChangeLink, linkName, linkSummary, partitionLinks } from './inviteLinksModel'
import { LinkShareBox } from './LinkShareBox'
import type { InviteLinksState, LinkPeople } from './useInviteLinks'
import { useInviteLinks } from './useInviteLinks'
import './inviteLinks.css'

type Page = { id: 'home' } | { id: 'create' } | { id: 'edit'; linkId: string } | { id: 'detail'; linkId: string }

const PAGE_TITLE: Record<Page['id'], string> = {
  home: '邀请链接',
  create: '新建链接',
  edit: '编辑链接',
  detail: '邀请链接',
}

export interface InviteLinksPanelProps {
  chatId: string
  api: InviteLinksService
  onClose(): void
  /** Test/screenshot seeds. */
  initial?: Partial<InviteLinksState> | undefined
  initialPage?: Page | undefined
  initialPeople?: LinkPeople | undefined
  origin?: string | undefined
}

function BackIcon() {
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
      <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  )
}

function LinkRow({ link, onOpen }: { link: InviteLink; onOpen(): void }) {
  return (
    <li>
      <button type="button" className="tg-invite__row" data-state={link.state} onClick={onOpen}>
        <span className="tg-invite__row-icon" aria-hidden="true">
          🔗
        </span>
        <span className="tg-invite__row-text">
          <span className="tg-invite__row-name">{linkName(link)}</span>
          <span className="tg-invite__meta">{linkSummary(link)}</span>
        </span>
      </button>
    </li>
  )
}

export function InviteLinksPanel({
  chatId,
  api,
  onClose,
  initial,
  initialPage,
  initialPeople,
  origin,
}: InviteLinksPanelProps) {
  const links = useInviteLinks(api, chatId, initial)
  const [stack, setStack] = useState<Page[]>(() =>
    initialPage && initialPage.id !== 'home' ? [{ id: 'home' }, initialPage] : [{ id: 'home' }],
  )
  const [confirmReplace, setConfirmReplace] = useState(false)
  const page = stack[stack.length - 1] ?? { id: 'home' }
  const open = (next: Page) => setStack((current) => [...current, next])
  const back = () => setStack((current) => (current.length > 1 ? current.slice(0, -1) : current))
  const home = () => setStack([{ id: 'home' }])
  const view = links.view
  const find = (linkId: string) => view?.links.find((link) => link.id === linkId) ?? null
  const viewerId = api.viewerId()
  const people = links.people
  const detailId = page.id === 'detail' ? page.linkId : null
  const loadPeople = useCallback(() => people(detailId ?? ''), [people, detailId])

  let body: ReactNode = null
  if (!view) {
    body = links.loading ? <Spinner label="正在加载" /> : null
  } else if (page.id === 'home') {
    const { primary, live, revoked } = partitionLinks(view.links)
    body = (
      <>
        <p className="tg-invite__note">任何拿到链接的人都可以加入群组；撤销后链接立即失效。</p>
        {primary ? (
          <section className="tg-invite__primary" aria-label="主邀请链接">
            <h3 className="tg-invite__section-title">主邀请链接</h3>
            <LinkShareBox token={primary.token} live origin={origin}>
              {confirmReplace ? null : (
                <Button variant="text" onClick={() => setConfirmReplace(true)}>
                  替换链接
                </Button>
              )}
            </LinkShareBox>
            {confirmReplace ? (
              <div className="tg-invite__confirm" role="group" aria-label="确认替换">
                <span>旧链接将立即失效。</span>
                <Button
                  variant="danger"
                  loading={links.busy}
                  onClick={() => void links.replacePrimary().then(() => setConfirmReplace(false))}
                >
                  替换
                </Button>
                <Button variant="text" onClick={() => setConfirmReplace(false)}>
                  取消
                </Button>
              </div>
            ) : null}
            <button
              type="button"
              className="tg-invite__stats"
              onClick={() => open({ id: 'detail', linkId: primary.id })}
            >
              {linkSummary(primary)}
            </button>
          </section>
        ) : null}
        <section aria-label="其他链接">
          <h3 className="tg-invite__section-title tg-invite__list-title">其他链接</h3>
          <div className="tg-invite__actions" data-start="">
            <Button variant="tonal" onClick={() => open({ id: 'create' })}>
              ＋ 创建新链接
            </Button>
          </div>
          {live.length === 0 ? (
            <p className="tg-invite__note">可以为不同渠道创建带有效期、人数上限或需要审核的链接。</p>
          ) : (
            <ul className="tg-invite__rows">
              {live.map((link) => (
                <LinkRow key={link.id} link={link} onOpen={() => open({ id: 'detail', linkId: link.id })} />
              ))}
            </ul>
          )}
        </section>
        {revoked.length > 0 ? (
          <section aria-label="已撤销的链接">
            <h3 className="tg-invite__section-title tg-invite__list-title">已撤销的链接</h3>
            <ul className="tg-invite__rows">
              {revoked.map((link) => (
                <LinkRow key={link.id} link={link} onOpen={() => open({ id: 'detail', linkId: link.id })} />
              ))}
            </ul>
          </section>
        ) : null}
      </>
    )
  } else if (page.id === 'create') {
    body = (
      <InviteLinkEditor busy={links.busy} onSave={(input) => void links.create(input).then((made) => made && home())} />
    )
  } else if (page.id === 'edit') {
    const link = find(page.linkId)
    body = link ? (
      <InviteLinkEditor
        link={link}
        busy={links.busy}
        onSave={(input) => void links.edit(link.id, input).then((ok) => ok && back())}
      />
    ) : null
  } else {
    const link = find(page.linkId)
    body = link ? (
      <InviteLinkDetail
        link={link}
        canChange={canChangeLink(link, viewerId, view.can_manage_others)}
        canReview={view.can_review}
        busy={links.busy}
        loadPeople={loadPeople}
        initialPeople={initialPeople}
        origin={origin}
        onEdit={() => open({ id: 'edit', linkId: link.id })}
        onRevoke={() => void links.revoke(link.id).then((ok) => ok && home())}
        onDelete={() => void links.remove(link.id).then((ok) => ok && home())}
        onSettle={(userId, approve) => links.settle(userId, approve)}
      />
    ) : (
      <p className="tg-invite__note">链接已不存在。</p>
    )
  }

  return (
    <section className="tg-invite" aria-label="邀请链接">
      <header className="tg-invite__bar">
        {stack.length > 1 ? (
          <IconButton label="返回" variant="plain" onClick={back}>
            <BackIcon />
          </IconButton>
        ) : null}
        <h2 className="tg-invite__heading">{PAGE_TITLE[page.id]}</h2>
        <Button variant="text" size="sm" onClick={onClose}>
          关闭
        </Button>
      </header>
      {links.error ? (
        <p className="tg-invite__error" role="alert">
          {links.error}
        </p>
      ) : null}
      <ScrollArea className="tg-invite__body" orientation="vertical" overlay>
        {body}
      </ScrollArea>
    </section>
  )
}
