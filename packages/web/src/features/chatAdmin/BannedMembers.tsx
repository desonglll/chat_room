/**
 * TG-1203: Telegram's «封禁并移出群组» on a member's page and the «已封禁的用户» list with
 * «解除封禁». Before this the panel could restrict a member but never ban one, and nothing
 * could lift a ban made elsewhere. The server decides (`members.ban`); these only ask.
 */
import { useEffect, useState } from 'react'
import type { ChatMembership } from '@tg/core'
import { authStore, listChatMembers, selectToken, updateChatMember } from '@tg/core'
import { Avatar, Button, Spinner } from '@tg/ui'
import { apiClient } from '../../app/client'
import { t } from '../../i18n/index'

const token = () => selectToken(authStore.getState()) || ''

export interface BanApi {
  ban(chatId: string, userId: string): Promise<unknown>
  unban(chatId: string, userId: string): Promise<unknown>
  banned(chatId: string): Promise<ChatMembership[]>
}

export const banApi: BanApi = {
  ban: (chatId, userId) => updateChatMember(apiClient, chatId, userId, token(), 'ban'),
  unban: (chatId, userId) => updateChatMember(apiClient, chatId, userId, token(), 'unban'),
  banned: async (chatId) =>
    (await listChatMembers(apiClient, chatId, token())).filter((member) => member.status === 'banned'),
}

/** The danger action under «限制成员»; `onBanned` returns to the roster. */
export function BanMemberButton({
  chatId,
  userId,
  onBanned,
  api = banApi,
}: {
  chatId: string
  userId: string
  onBanned(): void
  api?: BanApi
}) {
  const [state, setState] = useState<'idle' | 'confirm' | 'busy' | 'failed'>('idle')
  if (state === 'idle' || state === 'failed') {
    return (
      <>
        <Button variant="danger" onClick={() => setState('confirm')}>
          {t('w.chatAdmin.banMember')}
        </Button>
        {state === 'failed' ? (
          <p className="tg-chatadmin__error" role="alert">
            {t('w.chatAdmin.51d3cb')}
          </p>
        ) : null}
      </>
    )
  }
  return (
    <div className="tg-chatadmin__confirm" role="group" aria-label={t('w.chatAdmin.banMember')}>
      <p className="tg-chatadmin__note">{t('w.chatAdmin.banWarning')}</p>
      <Button
        variant="danger"
        loading={state === 'busy'}
        onClick={() => {
          setState('busy')
          api.ban(chatId, userId).then(onBanned, () => setState('failed'))
        }}
      >
        {t('w.chatAdmin.banConfirm')}
      </Button>
      <Button variant="text" disabled={state === 'busy'} onClick={() => setState('idle')}>
        {t('w.chatAdmin.banCancel')}
      </Button>
    </div>
  )
}

/** «已封禁的用户»: everyone banned from the chat, each with «解除封禁». */
export function BannedMembersPage({ chatId, api = banApi }: { chatId: string; api?: BanApi }) {
  const [people, setPeople] = useState<ChatMembership[] | null>(null)
  const [failed, setFailed] = useState(false)
  const [busy, setBusy] = useState('')

  useEffect(() => {
    let cancelled = false
    api.banned(chatId).then(
      (list) => !cancelled && setPeople(list),
      () => !cancelled && setFailed(true),
    )
    return () => {
      cancelled = true
    }
  }, [api, chatId])

  if (failed) return <p className="tg-chatadmin__note">{t('w.chatAdmin.d9f607')}</p>
  if (!people) return <Spinner label={t('w.chatAdmin.3667cb')} />
  if (people.length === 0) return <p className="tg-chatadmin__note">{t('w.chatAdmin.noBanned')}</p>
  return (
    <ul className="tg-chatadmin__members">
      {people.map((person) => {
        const name = person.nickname || person.username
        return (
          <li key={person.user_id} className="tg-chatadmin__member">
            <div className="tg-chatadmin__member-button">
              <Avatar label={name} initials={person.avatar_emoji || undefined} size="md" />
              <span className="tg-chatadmin__member-text">
                <span className="tg-chatadmin__member-name">{name}</span>
                <span className="tg-chatadmin__member-sub">@{person.username}</span>
              </span>
              <Button
                variant="text"
                size="sm"
                loading={busy === person.user_id}
                disabled={busy !== ''}
                onClick={() => {
                  setBusy(person.user_id)
                  api
                    .unban(chatId, person.user_id)
                    .then(
                      () => setPeople((list) => (list ?? []).filter((item) => item.user_id !== person.user_id)),
                      () => setFailed(true),
                    )
                    .finally(() => setBusy(''))
                }}
              >
                {t('w.chatAdmin.unban')}
              </Button>
            </div>
          </li>
        )
      })}
    </ul>
  )
}
