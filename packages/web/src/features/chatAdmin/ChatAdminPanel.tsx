/**
 * The chat administration panel (TG-201): Telegram's "管理群组" page and its sub-pages —
 * chat type, member permissions (group defaults), administrators, restricted members and the
 * paged roster, with the admin and restriction editors one level down.
 *
 * Navigation is a small in-panel stack; the host decides where the panel lives
 * (`ChatAdminEntry` slides it in as a right sheet over the info panel).
 */
import { useEffect, useState, type ReactNode } from 'react'
import type { ChatAdminApi, ChatMemberEntry } from '@tg/core'
import { Button, IconButton, ScrollArea, Spinner } from '@tg/ui'
import { ForumToggle } from '../forum/ForumToggle'
import { AdminEditor } from './AdminEditor'
import { BanMemberButton, BannedMembersPage } from './BannedMembers'
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
import { ChannelSignaturesToggle } from '../channel/ChannelSignaturesToggle'
import { useChatAdmin } from './useChatAdmin'
import { t } from '../../i18n/index'
import './chatAdmin.css'

type Page =
  | { id: 'home' }
  | { id: 'defaults' }
  | { id: 'admins' }
  | { id: 'restricted' }
  | { id: 'banned' }
  | { id: 'members' }
  | { id: 'member'; entry: ChatMemberEntry }
  | { id: 'admin'; entry: ChatMemberEntry }
  | { id: 'restrict'; entry: ChatMemberEntry }

const PAGE_TITLE: Record<Page['id'], string> = {
  get home() {
    return t('w.chatAdmin.924751')
  },
  get defaults() {
    return t('w.chatAdmin.24a78e')
  },
  get admins() {
    return t('w.chatAdmin.ef84e7')
  },
  get restricted() {
    return t('w.chatAdmin.15d54e')
  },
  get banned() {
    return t('w.chatAdmin.bannedTitle')
  },
  get members() {
    return t('w.chatAdmin.c1ee9f')
  },
  get member() {
    return t('w.chatAdmin.c1ee9f')
  },
  get admin() {
    return t('w.chatAdmin.df1ff6')
  },
  get restrict() {
    return t('w.chatAdmin.c7439b')
  },
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
  // TG-1203: slow mode, a public link or topics upgrade a group to a supergroup while this panel
  // is open. The `chat_updated` frame updates the chat list at once; the permissions view (read
  // when the panel opened) is re-read, and the live type is shown meanwhile.
  const liveType = useStore(chatListStore, (state) => state.chats.find((chat) => chat.id === chatId)?.chat_type)
  const loadedType = admin.view?.chat_type
  const { reload } = admin
  useEffect(() => {
    if (liveType && loadedType && liveType !== loadedType) void reload()
  }, [liveType, loadedType, reload])
  const view = admin.view && liveType ? { ...admin.view, chat_type: liveType } : admin.view
  const can = adminCapabilities(view)
  // TG-1203: a channel has subscribers, no member rules and no per-member restrictions.
  const isChannel = view?.chat_type === 'channel'
  const peopleLabel = isChannel ? t('w.chatAdmin.subscribers') : t('w.chatAdmin.c1ee9f')
  const heading =
    page.id === 'home' && isChannel
      ? t('w.chatAdmin.manageChannel')
      : page.id === 'members' && isChannel
        ? peopleLabel
        : PAGE_TITLE[page.id]

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
    body = admin.loading ? <Spinner label={t('w.chatAdmin.3667cb')} /> : null
  } else if (page.id === 'home') {
    body = (
      <>
        <section className="tg-chatadmin__type" aria-label={t('w.chatAdmin.b98234')}>
          <span className="tg-chatadmin__type-label">{CHAT_TYPE_LABEL[view.chat_type]}</span>
          <span className="tg-chatadmin__type-count">
            {view.member_count} {isChannel ? t('w.chatAdmin.subscriberUnit') : t('w.chatAdmin.b8d0b7')}
          </span>
          {chatTypeNote(view.chat_type) ? <p className="tg-chatadmin__note">{chatTypeNote(view.chat_type)}</p> : null}
        </section>
        <ul className="tg-chatadmin__navlist">
          {isChannel ? null : (
            <NavRow
              label={t('w.chatAdmin.24a78e')}
              value={`${view.default_permissions.length}/9`}
              onOpen={() => open({ id: 'defaults' })}
            />
          )}
          <NavRow
            label={t('w.chatAdmin.ef84e7')}
            value={String(admin.admins.length)}
            onOpen={() => open({ id: 'admins' })}
          />
          {can.ban && !isChannel ? (
            <NavRow
              label={t('w.chatAdmin.15d54e')}
              value={String(admin.restricted.length)}
              onOpen={() => open({ id: 'restricted' })}
            />
          ) : null}
          {can.ban ? (
            <NavRow label={t('w.chatAdmin.bannedTitle')} value="" onOpen={() => open({ id: 'banned' })} />
          ) : null}
          <NavRow label={peopleLabel} value={String(view.member_count)} onOpen={() => open({ id: 'members' })} />
        </ul>
        <ForumToggle chatId={chatId} chatType={view.chat_type} myPermissions={view.my_permissions} />
        <PublicLinkEditor
          chatId={chatId}
          chatType={view.chat_type}
          current={publicUsername}
          myPermissions={view.my_permissions}
        />
        {/* TG-1203: the signature switch existed only in the creation dialog. */}
        <ChannelSignaturesToggle chatId={chatId} chatType={view.chat_type} myPermissions={view.my_permissions} />
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
    body = list(admin.admins, t('w.chatAdmin.948163'))
  } else if (page.id === 'restricted') {
    body = list(admin.restricted, t('w.chatAdmin.d13ab5'))
  } else if (page.id === 'banned') {
    body = <BannedMembersPage chatId={chatId} />
  } else if (page.id === 'members') {
    body = (
      <>
        {list(admin.members, t('w.chatAdmin.5c64fe'))}
        {admin.membersDone ? null : (
          <div className="tg-chatadmin__actions">
            <Button variant="text" onClick={() => void admin.loadMoreMembers()}>
              {t('w.chatAdmin.3a0fab')}
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
              {t('w.chatAdmin.150282')}
            </Button>
          ) : null}
          {can.ban ? (
            <Button variant="tonal" onClick={() => open({ id: 'restrict', entry })}>
              {t('w.chatAdmin.c7439b')}
            </Button>
          ) : null}
          {can.ban ? (
            <BanMemberButton
              chatId={chatId}
              userId={entry.user_id}
              onBanned={() => {
                void admin.reload()
                setStack([{ id: 'home' }, { id: 'members' }])
              }}
            />
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
    <section className="tg-chatadmin" aria-label={isChannel ? t('w.chatAdmin.manageChannel') : t('w.chatAdmin.924751')}>
      <header className="tg-chatadmin__bar">
        {stack.length > 1 ? (
          <IconButton label={t('w.chatAdmin.11d024')} variant="plain" onClick={back}>
            <BackIcon />
          </IconButton>
        ) : null}
        <h2 className="tg-chatadmin__heading">{heading}</h2>
        <Button variant="text" size="sm" onClick={onClose}>
          {t('w.chatAdmin.6c14bd')}
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
