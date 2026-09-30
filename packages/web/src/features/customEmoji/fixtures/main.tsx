import '@tg/ui/styles.css'
import '../../../styles/index.css'
import { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { listChatMessages, storedMessageToBroadcast, type BroadcastMessage } from '@tg/core'
import { apiClient } from '../../../app/client'
import { browserStorage } from '../../../app/platform'
import { hydrateSession } from '../../../app/session'
import { authStore } from '@tg/core'
import { MessageBubble } from '../../message'
import { makeCtx } from '../../message/fixtures/bubbleFixtures'
import '../register'
import { CustomEmojiTab } from '../CustomEmojiTab'
import { EmojiStatus } from '../EmojiStatus'

const session = hydrateSession({ storage: browserStorage, store: authStore }, Date.now())
const chatId = new URLSearchParams(location.search).get('chat') ?? ''

function LiveHistory() {
  const [messages, setMessages] = useState<BroadcastMessage[]>([])
  const [picked, setPicked] = useState('')
  useEffect(() => {
    void listChatMessages(apiClient, chatId, { token: session?.token ?? '' }).then((page) =>
      setMessages(page.map(storedMessageToBroadcast)),
    )
  }, [])
  return (
    <div className="tg-custom-emoji-fixture">
      <h2 className="tg-custom-emoji-fixture__name">
        {session?.user.username}
        <EmojiStatus userId={session?.user.id ?? ''} />
      </h2>
      {messages.map((message) => (
        <MessageBubble key={message.message_id} message={message} ctx={makeCtx({ isOutgoing: true })} />
      ))}
      <CustomEmojiTab onPick={(emoji) => setPicked(`${emoji.emoji} ${emoji.id}`)} />
      <p className="tg-custom-emoji-fixture__picked">{picked}</p>
    </div>
  )
}

const host = document.getElementById('fixtures')
if (host === null) throw new Error('fixtures: #fixtures is missing from index.html')
createRoot(host).render(<LiveHistory />)
