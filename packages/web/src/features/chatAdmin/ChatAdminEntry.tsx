/**
 * The entry point the chat info panel mounts (TG-106 owns that panel; see the TG-201 devlog
 * integration patch list): a row showing the chat type and member count — "管理群组" for an
 * administrator, "群组权限" (read-only) for a member — which slides the admin panel in as a
 * right sheet.
 *
 *   <ChatAdminEntry chatId={chatId} />
 */
import { useState } from 'react'
import type { ChatAdminApi, ChatPermissionsView } from '@tg/core'
import { Sheet } from '@tg/ui'
import { chatAdminApi } from './chatAdminApi'
import { adminCapabilities, CHAT_TYPE_LABEL } from './chatAdminModel'
import { ChatAdminPanel } from './ChatAdminPanel'
import { useChatAdminAccess } from './useChatAdminAccess'
import './chatAdmin.css'

export interface ChatAdminEntryProps {
  chatId: string
  /** Defaults to the app-wide client; tests inject a fake. */
  api?: ChatAdminApi | undefined
  /** Test/screenshot seed for the access check. */
  initialView?: ChatPermissionsView | undefined
}

function ShieldIcon() {
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
      <path
        d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6l7-3z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export function ChatAdminEntry({ chatId, api = chatAdminApi, initialView }: ChatAdminEntryProps) {
  const [open, setOpen] = useState(false)
  const [reloadKey, setReloadKey] = useState(0)
  const access = useChatAdminAccess(api, initialView ? null : chatId, reloadKey)
  const view = initialView ?? access.view
  // Channels get their own admin surface (TG-202); a private chat has nothing to administer.
  if (!view || view.chat_type === 'private' || view.chat_type === 'channel') return null
  // Every member sees the chat type and the rules; administrators also get the editors.
  const manages = adminCapabilities(view).any

  return (
    <>
      <button type="button" className="tg-chatadmin__entry" onClick={() => setOpen(true)}>
        <span className="tg-chatadmin__entry-icon">
          <ShieldIcon />
        </span>
        <span className="tg-chatadmin__entry-text">
          <span className="tg-chatadmin__entry-title">{manages ? '管理群组' : '群组权限'}</span>
          <span className="tg-chatadmin__entry-sub">
            {CHAT_TYPE_LABEL[view.chat_type]} · {view.member_count} 位成员
          </span>
        </span>
      </button>
      <Sheet
        open={open}
        side="right"
        ariaLabel="管理群组"
        showClose={false}
        className="tg-chatadmin__sheet"
        onClose={() => {
          setOpen(false)
          setReloadKey((key) => key + 1)
        }}
      >
        {open ? (
          <ChatAdminPanel
            chatId={chatId}
            api={api}
            onClose={() => {
              setOpen(false)
              setReloadKey((key) => key + 1)
            }}
          />
        ) : null}
      </Sheet>
    </>
  )
}
