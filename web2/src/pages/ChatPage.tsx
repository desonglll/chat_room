import { MessageOutlined } from '@ant-design/icons'
import { App, Empty } from 'antd'
import { useCallback, useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { useAuth } from '../features/auth/auth-context'
import { ConversationList } from '../features/chat/ConversationList'
import { SelectedRoom } from '../features/chat/SelectedRoom'
import { endpoints, errorMessage } from '../lib/api'
import type { Conversation } from '../types'

export function ChatPage() {
  const { roomId } = useParams()
  const { session, user } = useAuth()
  const { message } = App.useApp()
  const [conversations, setConversations] = useState<Conversation[]>([])
  const [loading, setLoading] = useState(true)

  const loadConversations = useCallback(async () => {
    try {
      setConversations(await endpoints.conversations())
    } catch (error) {
      message.error(errorMessage(error, '无法加载会话'))
    } finally {
      setLoading(false)
    }
  }, [message])

  useEffect(() => {
    let active = true
    void endpoints
      .conversations()
      .then((data) => active && setConversations(data))
      .catch((error) => message.error(errorMessage(error, '无法加载会话')))
      .finally(() => active && setLoading(false))
    const timer = window.setInterval(() => void loadConversations(), 15_000)
    return () => {
      active = false
      window.clearInterval(timer)
    }
  }, [loadConversations, message])

  if (!session || !user) return null

  return (
    <div className="flex h-full min-h-0">
      <div className={`${roomId ? 'hidden md:flex' : 'flex'} h-full min-h-0`}>
        <ConversationList
          conversations={conversations}
          selectedId={roomId}
          loading={loading}
        />
      </div>

      <div className={`${roomId ? 'flex' : 'hidden md:flex'} min-w-0 flex-1`}>
        {roomId ? (
          <SelectedRoom key={roomId} roomId={roomId} session={session} user={user} />
        ) : (
          <div className="flex flex-1 items-center justify-center bg-[#f8faf9] px-6">
            <Empty
              image={<MessageOutlined className="text-5xl text-[#88a097]" />}
              description="选择一个会话开始聊天"
            />
          </div>
        )}
      </div>
    </div>
  )
}
