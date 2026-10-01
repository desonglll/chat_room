/**
 * One `ComposerController` + one TG-107 chat-action sender per (chat, session). On a
 * chat switch the old sender announces `cancel` (the user stopped typing there) and
 * drops its keep-alives before the new one takes over.
 */
import { useEffect, useMemo } from 'react'
import { authStore, composerStore, createChatActionSender, forwardMessages, messageStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { browserClock } from '../../app/platform'
import type { ComposerController, ComposerSessionApi } from './composerController'
import { createComposerController } from './composerController'
import { reportForwardResults } from './forwardNotice'

export function useComposerController(chatId: string, session: ComposerSessionApi): ComposerController {
  const actions = useMemo(
    () => createChatActionSender({ clock: browserClock, send: (_chatId, frame) => session.sendFrame(frame) }),
    [session],
  )

  useEffect(
    () => () => {
      actions.sendChatAction(chatId, 'cancel')
      actions.dispose()
    },
    [actions, chatId],
  )

  return useMemo(
    () =>
      createComposerController({
        chatId,
        session,
        composer: composerStore,
        messages: messageStore,
        actions,
        forward: (messageIds, targetChatId) =>
          forwardMessages(apiClient, selectToken(authStore.getState()), [...messageIds], [targetChatId]).then(
            (results) => reportForwardResults(targetChatId, results),
          ),
      }),
    [actions, chatId, session],
  )
}
