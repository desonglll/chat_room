/**
 * The chat administration panel (TG-201): Telegram's "管理群组" page and its sub-pages —
 * chat type, member permissions (group defaults), administrators, restricted members and the
 * paged roster, with the admin and restriction editors one level down.
 *
 * Navigation is a small in-panel stack; the host decides where the panel lives
 * (`ChatAdminEntry` slides it in as a right sheet over the info panel).
 */
import { useState, type ReactNode } from 'react'
import type { ChatAdminApi, ChatMemberEntry } from '@tg/core'
import { Button, IconButton, ScrollArea, Spinner } from '@tg/ui'
import { ForumToggle } from '../forum/ForumToggle'
import { AdminEditor } from './AdminEditor'
import { adminCapabilities, CHAT_TYPE_LABEL, chatTypeNote, memberName } from './chatAdminModel'
import { DefaultPermissionsPage } from './DefaultPermissionsPage'
import { PublicLinkEditor } from '../chatPreview/PublicLinkEditor'
import { useStore } from 'zustand/react'
import { chatListStore } from '@tg/core'
import { SlowModePicker } from './slowMode/SlowModePicker'
import { MemberRow } from './MemberRow'
import { RestrictionEditor } from './RestrictionEditor'
import type { ChatAdminState } from './useChatAdmin'
import { InviteLinksEntry } from '../inviteLinks/InviteLinksEntry'
import { DiscussionLinkEditor } from '../channel/comments/DiscussionLinkEditor'
import { useChatAdmin } from './useChatAdmin'
import './chatAdmin.css'

type Page =
  | { id: 'home' }
  | { id: 'defaults' }
  | { id: 'admins' }
  | { id: 'restricted' }
  | { id: 'members' }
  | { id: 'member'; entry: ChatMemberEntry }
  | { id: 'admin'; entry: ChatMemberEntry }
  | { id: 'restrict'; entry: ChatMemberEntry }

const PAGE_TITLE: Record<Page['id'], string> = {
  home: '管理群组',
  defaults: '成员权限',
  admins: '管理员',
  restricted: '被限制的成员',
  members: '成员',
  member: '成员',
  admin: '管理员权限',
  restrict: '限制成员',
}

export interface ChatAdminPanelProps {
  chatId: string
  api: ChatAdminApi
  onClose(): void
  /** Test/screenshot seed; skips the initial fetch when it carries a `view`. */
  initial?: Partial<ChatAdminState> | undefined
  /** Test/screenshot: open on a sub-page. */
  initialPage?: 'home' | 'defaults' | 'admins' | 'restricted' | 'members' | undefined
}

function BackIcon() {
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
      <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  )
}

function NavRow({ label, value, onOpen }: { label: string; value: string; onOpen(): void }) {
  return (
    <li>
      <button type="button" className="tg-chatadmin__nav" onClick={onOpen}>
        <span>{label}</span>
        <span className="tg-chatadmin__nav-value">{value}</span>
      </button>
    </li>
  )
}

export function ChatAdminPanel({ chatId, api, onClose, initial, initialPage = 'home' }: ChatAdminPanelProps) {
  const admin = useChatAdmin(api, chatId, initial)
  // TG-206: the chat's current public handle, kept live by `chat_updated` frames.
  const publicUsername = useStore(
    chatListStore,
    (state) => state.chats.find((chat) => chat.id === chatId)?.username ?? null,
  )
  const [stack, setStack] = useState<Page[]>(() =>
    initialPage === 'home' ? [{ id: 'home' }] : [{ id: 'home' }, { id: initialPage }],
  )
  const page = stack[stack.length - 1] ?? { id: 'home' }
  const open = (next: Page) => setStack((current) => [...current, next])
  const back = () => setStack((current) => (current.length > 1 ? current.slice(0, -1) : current))
  const view = admin.view
  const can = adminCapabilities(view)

  const afterWrite = (ok: boolean) => {
    if (ok) back()
  }

  const openMember = (entry: ChatMemberEntry) => {
    if (entry.role === 'owner') return undefined
    if (entry.role === 'admin') return can.manageAdmins ? () => open({ id: 'admin', entry }) : undefined
    return can.promote || can.ban ? () => open({ id: 'member', entry }) : undefined
  }

  const list = (entries: readonly ChatMemberEntry[], empty: string) =>
    entries.length === 0 ? (
      <p className="tg-chatadmin__note">{empty}</p>
    ) : (
      <ul className="tg-chatadmin__members">
        {entries.map((entry) => (
          <MemberRow key={entry.user_id} entry={entry} onOpen={openMember(entry)} />
        ))}
      </ul>
    )

  let body: ReactNode = null
  if (!view) {
    body = admin.loading ? <Spinner label="正在加载" /> : null
  } else if (page.id === 'home') {
    body = (
      <>
        <section className="tg-chatadmin__type" aria-label="群类型">
          <span className="tg-chatadmin__type-label">{CHAT_TYPE_LABEL[view.chat_type]}</span>
          <span className="tg-chatadmin__type-count">{view.member_count} 位成员</span>
          {chatTypeNote(view.chat_type) ? <p className="tg-chatadmin__note">{chatTypeNote(view.chat_type)}</p> : null}
        </section>
        <ul className="tg-chatadmin__navlist">
          <NavRow
            label="成员权限"
            value={`${view.default_permissions.length}/9`}
            onOpen={() => open({ id: 'defaults' })}
          />
          <NavRow label="管理员" value={String(admin.admins.length)} onOpen={() => open({ id: 'admins' })} />
          {can.ban ? (
            <NavRow
              label="被限制的成员"
              value={String(admin.restricted.length)}
              onOpen={() => open({ id: 'restricted' })}
            />
          ) : null}
          <NavRow label="成员" value={String(view.member_count)} onOpen={() => open({ id: 'members' })} />
        </ul>
        <ForumToggle chatId={chatId} chatType={view.chat_type} myPermissions={view.my_permissions} />
        <PublicLinkEditor
          chatId={chatId}
          chatType={view.chat_type}
          current={publicUsername}
          myPermissions={view.my_permissions}
        />
        {/* TG-203: a channel's discussion group (comments). */}
        <DiscussionLinkEditor chatId={chatId} chatType={view.chat_type} myPermissions={view.my_permissions} />
        {/* TG-205: `overlay` — a second sheet inside this one fights it for focus. */}
        <InviteLinksEntry chatId={chatId} variant="overlay" />
      </>
    )
  } else if (page.id === 'defaults') {
    body = (
      <DefaultPermissionsPage
        view={view}
        busy={admin.busy}
        editable={can.ban}
        onSave={(permissions) => void admin.setDefaults(permissions).then(afterWrite)}
      />
    )
    // TG-207: Telegram shows slow mode under the group's permissions, for admins who restrict.
    if (can.ban && view.chat_type !== 'channel' && view.chat_type !== 'private') {
      body = (
        <>
          {body}
          <SlowModePicker chatId={chatId} />
        </>
      )
    }
  } else if (page.id === 'admins') {
    body = list(admin.admins, '还没有管理员。')
  } else if (page.id === 'restricted') {
    body = list(admin.restricted, '没有被限制的成员。')
  } else if (page.id === 'members') {
    body = (
      <>
        {list(admin.members, '还没有成员。')}
        {admin.membersDone ? null : (
          <div className="tg-chatadmin__actions">
            <Button variant="text" onClick={() => void admin.loadMoreMembers()}>
              加载更多
            </Button>
          </div>
        )}
      </>
    )
  } else if (page.id === 'member') {
    const entry = page.entry
    body = (
      <div className="tg-chatadmin__editor">
        <p className="tg-chatadmin__lead">
          <strong>{memberName(entry)}</strong>
        </p>
        <div className="tg-chatadmin__actions" data-stack="">
          {can.promote ? (
            <Button variant="tonal" onClick={() => open({ id: 'admin', entry })}>
              设为管理员
            </Button>
          ) : null}
          {can.ban ? (
            <Button variant="tonal" onClick={() => open({ id: 'restrict', entry })}>
              限制成员
            </Button>
          ) : null}
        </div>
      </div>
    )
  } else if (page.id === 'admin') {
    const entry = page.entry
    body = (
      <AdminEditor
        target={entry}
        actor={view}
        busy={admin.busy}
        onSave={(permissions, title) =>
          void admin
            .appoint(entry.user_id, permissions, title)
            .then((ok) => ok && setStack([{ id: 'home' }, { id: 'admins' }]))
        }
        onDismiss={
          can.manageAdmins
            ? () => void admin.dismiss(entry.user_id).then((ok) => ok && setStack([{ id: 'home' }, { id: 'admins' }]))
            : undefined
        }
      />
    )
  } else if (page.id === 'restrict') {
    const entry = page.entry
    body = (
      <RestrictionEditor
        target={entry}
        view={view}
        busy={admin.busy}
        onSave={(denied, until) =>
          void admin
            .restrict(entry.user_id, denied, until)
            .then((ok) => ok && setStack([{ id: 'home' }, { id: 'restricted' }]))
        }
      />
    )
  }

  return (
    <section className="tg-chatadmin" aria-label="管理群组">
      <header className="tg-chatadmin__bar">
        {stack.length > 1 ? (
          <IconButton label="返回" variant="plain" onClick={back}>
            <BackIcon />
          </IconButton>
        ) : null}
        <h2 className="tg-chatadmin__heading">{PAGE_TITLE[page.id]}</h2>
        <Button variant="text" size="sm" onClick={onClose}>
          关闭
        </Button>
      </header>
      {admin.error ? (
        <p className="tg-chatadmin__error" role="alert">
          {admin.error}
        </p>
      ) : null}
      <ScrollArea className="tg-chatadmin__body" orientation="vertical" overlay>
        {body}
      </ScrollArea>
    </section>
  )
}
