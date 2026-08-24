import { useCallback, useEffect, useRef, useState } from 'react'
import { websocketUrl } from '../../lib/api'
import type { Room, RoomMember, RoomSocketEvent, StoredMessage } from '../../types'

type SocketStatus = 'idle' | 'connecting' | 'ready' | 'error'

function sortMessages(messages: StoredMessage[]) {
  return [...messages].sort(
    (left, right) =>
      new Date(left.created_at).getTime() - new Date(right.created_at).getTime() ||
      left.id.localeCompare(right.id),
  )
}

function mergeMessages(current: StoredMessage[], incoming: StoredMessage[]) {
  const byId = new Map(current.map((message) => [message.id, message]))
  incoming.forEach((message) => byId.set(message.id, message))
  return sortMessages([...byId.values()])
}

function broadcastToMessage(event: Extract<RoomSocketEvent, { type: 'broadcast' }>, roomId: string) {
  const { type: _type, message_id, timestamp, ...message } = event
  return { ...message, id: message_id, room_id: roomId, created_at: timestamp } as StoredMessage
}

export function useRoomSocket(room: Room | null, token: string, password?: string) {
  const [messages, setMessages] = useState<StoredMessage[]>([])
  const [members, setMembers] = useState<RoomMember[]>([])
  const [participants, setParticipants] = useState<RoomMember[]>([])
  const [status, setStatus] = useState<SocketStatus>('idle')
  const [error, setError] = useState('')
  const [typingUser, setTypingUser] = useState('')
  const [notice, setNotice] = useState('')
  const socketRef = useRef<WebSocket | null>(null)
  const typingTimerRef = useRef<number | undefined>(undefined)

  useEffect(() => {
    if (!room || (room.has_password && password === undefined)) {
      return
    }

    let active = true
    let reconnectTimer: number | undefined

    const connect = () => {
      if (!active) return
      setStatus('connecting')
      const socket = new WebSocket(websocketUrl(`/ws/${room.id}`))
      socketRef.current = socket

      socket.onopen = () => {
        socket.send(
          JSON.stringify(
            room.has_password
              ? { type: 'auth', token, password: password ?? '' }
              : { type: 'join', token },
          ),
        )
      }

      socket.onmessage = (frame) => {
        let event: RoomSocketEvent
        try {
          event = JSON.parse(frame.data as string) as RoomSocketEvent
        } catch {
          return
        }

        if (event.type === 'auth_ok') {
          setMembers(event.members)
          setParticipants(event.participants)
          setError('')
          return
        }
        if (event.type === 'history_complete') {
          setStatus('ready')
          return
        }
        if (event.type === 'auth_fail') {
          setError(event.reason)
          setStatus('error')
          active = false
          socket.close()
          return
        }
        if (event.type === 'broadcast') {
          setMessages((current) => mergeMessages(current, [broadcastToMessage(event, room.id)]))
          return
        }
        if (event.type === 'message_edited') {
          setMessages((current) =>
            current.map((message) =>
              message.id === event.message_id
                ? { ...message, content: event.content, edited_at: event.edited_at }
                : message,
            ),
          )
          return
        }
        if (event.type === 'message_recalled') {
          setMessages((current) =>
            current.map((message) =>
              message.id === event.message_id
                ? { ...message, recalled_at: event.recalled_at }
                : message,
            ),
          )
          return
        }
        if (event.type === 'reaction_changed') {
          setMessages((current) =>
            current.map((message) => {
              if (message.id !== event.message_id) return message
              const reactions = message.reactions.map((reaction) => ({
                ...reaction,
                user_ids: [...reaction.user_ids],
              }))
              const existing = reactions.find((reaction) => reaction.emoji === event.emoji)
              if (existing && event.active && !existing.user_ids.includes(event.user_id)) {
                existing.user_ids.push(event.user_id)
              } else if (existing && !event.active) {
                existing.user_ids = existing.user_ids.filter((id) => id !== event.user_id)
              } else if (!existing && event.active) {
                reactions.push({ emoji: event.emoji, user_ids: [event.user_id] })
              }
              return { ...message, reactions: reactions.filter((reaction) => reaction.user_ids.length) }
            }),
          )
          return
        }
        if (event.type === 'presence') {
          setMembers(event.members)
          setParticipants(event.participants)
          return
        }
        if (event.type === 'system') {
          if (event.members) setMembers(event.members)
          if (event.participants) setParticipants(event.participants)
          setNotice(event.content)
          window.setTimeout(() => setNotice(''), 3_000)
          return
        }
        if (event.type === 'typing' && event.user_id) {
          setTypingUser(event.content ? event.username ?? '' : '')
          window.clearTimeout(typingTimerRef.current)
          typingTimerRef.current = window.setTimeout(() => setTypingUser(''), 2_000)
        }
      }

      socket.onerror = () => setError('实时连接出现异常')
      socket.onclose = () => {
        if (!active) return
        setStatus('connecting')
        reconnectTimer = window.setTimeout(connect, 1_500)
      }
    }

    connect()
    return () => {
      active = false
      window.clearTimeout(reconnectTimer)
      window.clearTimeout(typingTimerRef.current)
      socketRef.current?.close()
      socketRef.current = null
    }
  }, [password, room, token])

  const send = useCallback((payload: object) => {
    if (socketRef.current?.readyState !== WebSocket.OPEN) return false
    socketRef.current.send(JSON.stringify(payload))
    return true
  }, [])

  const prependMessages = useCallback((older: StoredMessage[]) => {
    setMessages((current) => mergeMessages(current, older))
  }, [])
  const addMessage = useCallback((message: StoredMessage) => {
    setMessages((current) => mergeMessages(current, [message]))
  }, [])
  const sendMessage = useCallback(
    (content: string, replyTo?: string) =>
      send({ type: 'message', content, reply_to: replyTo, client_message_id: crypto.randomUUID() }),
    [send],
  )
  const sendTyping = useCallback((content: string) => send({ type: 'typing', content }), [send])
  const sendRead = useCallback((messageId: string) => send({ type: 'read', message_id: messageId }), [send])
  const editMessage = useCallback(
    (messageId: string, content: string) => send({ type: 'edit', message_id: messageId, content }),
    [send],
  )
  const recallMessage = useCallback(
    (messageId: string) => send({ type: 'recall', message_id: messageId }),
    [send],
  )
  const reactToMessage = useCallback(
    (messageId: string, emoji: string, active: boolean) =>
      send({ type: 'reaction', message_id: messageId, emoji, active }),
    [send],
  )

  return {
    messages,
    members,
    participants,
    status,
    error,
    typingUser,
    notice,
    prependMessages,
    addMessage,
    sendMessage,
    sendTyping,
    sendRead,
    editMessage,
    recallMessage,
    reactToMessage,
  }
}
