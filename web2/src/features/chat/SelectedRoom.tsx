import { App, Input, Modal, Spin } from 'antd'
import { useCallback, useEffect, useState } from 'react'
import { endpoints, errorMessage } from '../../lib/api'
import type { AuthSession, Room, User } from '../../types'
import { RoomWorkspace } from './RoomWorkspace'

function passwordKey(roomId: string) {
  return `qiyu.room-password.${roomId}`
}

export function SelectedRoom({
  roomId,
  session,
  user,
}: {
  roomId: string
  session: AuthSession
  user: User
}) {
  const { message } = App.useApp()
  const [room, setRoom] = useState<Room | null>(null)
  const [password, setPassword] = useState<string | undefined>()
  const [passwordDraft, setPasswordDraft] = useState('')
  const [passwordOpen, setPasswordOpen] = useState(false)

  useEffect(() => {
    endpoints
      .room(roomId)
      .then((nextRoom) => {
        setRoom(nextRoom)
        if (nextRoom.has_password) {
          const stored = sessionStorage.getItem(passwordKey(nextRoom.id))
          if (stored !== null) setPassword(stored)
          else setPasswordOpen(true)
        }
      })
      .catch((error) => message.error(errorMessage(error, '无法打开房间')))
  }, [message, roomId])

  const acceptPassword = useCallback(() => {
    if (!room || password === undefined) return
    sessionStorage.setItem(passwordKey(room.id), password)
    setPasswordOpen(false)
  }, [password, room])

  const rejectPassword = useCallback(() => {
    if (!room) return
    sessionStorage.removeItem(passwordKey(room.id))
    setPassword(undefined)
    setPasswordDraft('')
    setPasswordOpen(true)
  }, [room])

  if (!room) {
    return <div className="flex flex-1 items-center justify-center"><Spin /></div>
  }

  return (
    <>
      <RoomWorkspace
        key={`${room.id}:${password ?? 'locked'}`}
        room={room}
        token={session.token}
        currentUserId={user.id}
        password={password}
        onPasswordAccepted={acceptPassword}
        onPasswordRejected={rejectPassword}
      />
      <Modal
        title="输入房间密码"
        open={passwordOpen}
        closable={false}
        maskClosable={false}
        okText="进入房间"
        cancelText="取消"
        okButtonProps={{ disabled: passwordDraft.length === 0 }}
        onOk={() => setPassword(passwordDraft)}
        onCancel={() => window.history.back()}
      >
        <Input.Password
          autoFocus
          value={passwordDraft}
          placeholder="房间密码"
          maxLength={256}
          onChange={(event) => setPasswordDraft(event.target.value)}
          onPressEnter={() => passwordDraft && setPassword(passwordDraft)}
        />
        <p className="mt-3 mb-0 text-xs text-[#79857f]">
          密码仅在当前标签页保存
        </p>
      </Modal>
    </>
  )
}
