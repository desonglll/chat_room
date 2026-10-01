/** TG-701: «移出» on a member row, for owners and admins (the server decides). */
import { useState } from 'react'
import type { ChatMembership } from '@tg/core'
import { authStore, chatListStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { t } from '../../i18n/index'
import { canRemove } from './lifecycleModel'
import { removeMember } from './lifecycleApi'

export function MemberRemove({ chatId, member }: { chatId: string; member: ChatMembership }) {
  const role = useStore(chatListStore, (state) => state.chats.find((chat) => chat.id === chatId)?.membership_role)
  const selfId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const [state, setState] = useState<'idle' | 'busy' | 'removed' | 'failed'>('idle')
  if (!canRemove(role, member.role, member.user_id === selfId)) return null
  if (state === 'removed') return <span className="tg-chatinfo__item-role">{t('w.lifecycle.removed')}</span>
  return (
    <button
      type="button"
      className="tg-chatinfo__item-remove"
      disabled={state === 'busy'}
      aria-label={t('w.lifecycle.removeMember', member.nickname || member.username)}
      onClick={() => {
        setState('busy')
        removeMember(chatId, member.user_id).then(
          () => setState('removed'),
          () => setState('failed'),
        )
      }}
    >
      {state === 'failed' ? t('w.lifecycle.retry') : t('w.lifecycle.remove')}
    </button>
  )
}
