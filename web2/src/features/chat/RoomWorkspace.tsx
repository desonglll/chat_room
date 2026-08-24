import {
  ArrowLeftOutlined,
  LockOutlined,
  TeamOutlined,
  UserAddOutlined,
} from '@ant-design/icons'
import { App, Badge, Button, Drawer, Form, Input, List, Modal, Spin, Tag, Tooltip } from 'antd'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { UserAvatar } from '../../components/UserAvatar'
import { endpoints, errorMessage } from '../../lib/api'
import type { PublicConfig, Room, RoomMembership, StoredMessage } from '../../types'
import { MessageComposer } from './MessageComposer'
import { MessageList } from './MessageList'
import { useRoomSocket } from './useRoomSocket'

interface RoomWorkspaceProps {
  room: Room
  token: string
  currentUserId: string
  password?: string
  onPasswordAccepted: () => void
  onPasswordRejected: () => void
}

export function RoomWorkspace({
  room,
  token,
  currentUserId,
  password,
  onPasswordAccepted,
  onPasswordRejected,
}: RoomWorkspaceProps) {
  const { message, modal } = App.useApp()
  const navigate = useNavigate()
  const socket = useRoomSocket(room, token, password)
  const [replyTo, setReplyTo] = useState<StoredMessage | null>(null)
  const [editTarget, setEditTarget] = useState<StoredMessage | null>(null)
  const [editValue, setEditValue] = useState('')
  const [olderLoading, setOlderLoading] = useState(false)
  const [canLoadOlder, setCanLoadOlder] = useState(true)
  const [membersOpen, setMembersOpen] = useState(false)
  const [memberships, setMemberships] = useState<RoomMembership[]>([])
  const [membersLoading, setMembersLoading] = useState(false)
  const [config, setConfig] = useState<PublicConfig | null>(null)

  useEffect(() => {
    void endpoints.config().then(setConfig).catch(() => undefined)
  }, [])

  useEffect(() => {
    if (socket.status === 'ready' && room.has_password) onPasswordAccepted()
  }, [onPasswordAccepted, room.has_password, socket.status])

  useEffect(() => {
    if (socket.status === 'error' && room.has_password) onPasswordRejected()
  }, [onPasswordRejected, room.has_password, socket.status])

  const loadOlder = async () => {
    const first = socket.messages[0]
    if (!first) return
    setOlderLoading(true)
    try {
      const older = await endpoints.messages(room.id, first.id, password)
      socket.prependMessages(older)
      setCanLoadOlder(older.length === 60)
    } catch (error) {
      message.error(errorMessage(error, '无法加载更早的消息'))
    } finally {
      setOlderLoading(false)
    }
  }

  const openMembers = async () => {
    setMembersOpen(true)
    setMembersLoading(true)
    try {
      setMemberships(await endpoints.members(room.id))
    } catch (error) {
      message.error(errorMessage(error, '无法加载成员'))
    } finally {
      setMembersLoading(false)
    }
  }

  const invite = async ({ username }: { username: string }) => {
    try {
      await endpoints.invite(room.id, username)
      message.success('邀请已发送')
    } catch (error) {
      message.error(errorMessage(error, '邀请失败'))
    }
  }

  const confirmRecall = (target: StoredMessage) => {
    modal.confirm({
      title: '撤回这条消息？',
      content: '撤回后其他成员将无法查看其内容。',
      okText: '撤回',
      okButtonProps: { danger: true },
      cancelText: '取消',
      onOk: () => socket.recallMessage(target.id),
    })
  }

  return (
    <section className="flex h-full min-w-0 flex-1 flex-col bg-white">
      <header className="flex h-[74px] shrink-0 items-center gap-3 border-b border-[#dfe5e2] px-3 sm:px-5">
        <Button
          type="text"
          className="md:hidden"
          icon={<ArrowLeftOutlined />}
          aria-label="返回会话列表"
          onClick={() => navigate('/chat')}
        />
        <UserAvatar emoji={room.avatar_emoji} name={room.name} size={42} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h2 className="m-0 truncate text-base font-semibold text-[#17201d]">{room.name}</h2>
            {room.has_password && <LockOutlined className="text-xs text-[#9a6a25]" />}
          </div>
          <p className="m-0 mt-0.5 truncate text-xs text-[#74817b]">
            {socket.status === 'ready'
              ? socket.typingUser
                ? `${socket.typingUser} 正在输入…`
                : `${socket.members.length} 人在线 · ${socket.participants.length} 位成员`
              : socket.status === 'error'
                ? socket.error
                : '正在连接…'}
          </p>
        </div>
        {socket.notice && <Tag color="green">{socket.notice}</Tag>}
        <Tooltip title="成员">
          <Badge count={room.membership_role === 'owner' ? 0 : undefined}>
            <Button type="text" icon={<TeamOutlined />} aria-label="房间成员" onClick={() => void openMembers()} />
          </Badge>
        </Tooltip>
      </header>

      <MessageList
        messages={socket.messages}
        currentUserId={currentUserId}
        loading={socket.status === 'connecting' || olderLoading}
        canLoadOlder={canLoadOlder && socket.messages.length > 0}
        onLoadOlder={() => void loadOlder()}
        onReply={setReplyTo}
        onEdit={(target) => {
          setEditTarget(target)
          setEditValue(target.content)
        }}
        onRecall={confirmRecall}
        onReact={(target, emoji, active) => socket.reactToMessage(target.id, emoji, active)}
        onRead={socket.sendRead}
      />

      <MessageComposer
        roomId={room.id}
        roomPassword={password}
        replyTo={replyTo}
        disabled={socket.status !== 'ready'}
        aiEnabled={Boolean(config?.ai_enabled)}
        onClearReply={() => setReplyTo(null)}
        onSendText={socket.sendMessage}
        onTyping={socket.sendTyping}
        onUploaded={socket.addMessage}
      />

      <Modal
        title="编辑消息"
        open={Boolean(editTarget)}
        okText="保存"
        cancelText="取消"
        okButtonProps={{ disabled: !editValue.trim() }}
        onCancel={() => setEditTarget(null)}
        onOk={() => {
          if (editTarget && socket.editMessage(editTarget.id, editValue.trim())) setEditTarget(null)
        }}
      >
        <Input.TextArea value={editValue} maxLength={4096} autoSize={{ minRows: 3, maxRows: 8 }} onChange={(event) => setEditValue(event.target.value)} />
      </Modal>

      <Drawer title="房间成员" width={390} open={membersOpen} onClose={() => setMembersOpen(false)}>
        {['owner', 'admin'].includes(room.membership_role ?? '') && (
          <Form layout="inline" className="mb-5 flex-nowrap" onFinish={invite}>
            <Form.Item name="username" className="min-w-0 flex-1" rules={[{ required: true, message: '请输入用户名' }]}>
              <Input prefix={<UserAddOutlined />} placeholder="邀请用户名" />
            </Form.Item>
            <Button type="primary" htmlType="submit">邀请</Button>
          </Form>
        )}
        {membersLoading ? (
          <div className="py-20 text-center"><Spin /></div>
        ) : (
          <List
            dataSource={memberships}
            renderItem={(member) => (
              <List.Item extra={<Tag>{roleLabel(member.role)}</Tag>}>
                <List.Item.Meta
                  avatar={<UserAvatar emoji={member.avatar_emoji} name={member.username} size={40} />}
                  title={member.nickname || member.username}
                  description={`@${member.username}`}
                />
              </List.Item>
            )}
          />
        )}
      </Drawer>
    </section>
  )
}

function roleLabel(role: string) {
  return { owner: '群主', admin: '管理员', member: '成员' }[role] ?? role
}
