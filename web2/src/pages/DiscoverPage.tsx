import {
  ArrowRightOutlined,
  LockOutlined,
  PlusOutlined,
  ReloadOutlined,
  SearchOutlined,
  TeamOutlined,
} from '@ant-design/icons'
import { App, Button, Empty, Form, Input, List, Modal, Radio, Spin, Tag } from 'antd'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { PageHeader } from '../components/PageHeader'
import { UserAvatar } from '../components/UserAvatar'
import { endpoints, errorMessage } from '../lib/api'
import type { Room } from '../types'

interface CreateRoomValues {
  name: string
  avatar_emoji?: string
  description?: string
  password?: string
  join_policy: 'open' | 'approval'
}

export function DiscoverPage() {
  const { message } = App.useApp()
  const navigate = useNavigate()
  const [rooms, setRooms] = useState<Room[]>([])
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(true)
  const [createOpen, setCreateOpen] = useState(false)
  const [creating, setCreating] = useState(false)
  const [joiningId, setJoiningId] = useState('')
  const [form] = Form.useForm<CreateRoomValues>()

  const load = useCallback(async () => {
    try {
      setRooms(await endpoints.rooms())
    } catch (error) {
      message.error(errorMessage(error, '无法加载房间'))
    } finally {
      setLoading(false)
    }
  }, [message])

  useEffect(() => {
    let active = true
    void endpoints
      .rooms()
      .then((data) => active && setRooms(data))
      .catch((error) => message.error(errorMessage(error, '无法加载房间')))
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [message])

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return rooms
    return rooms.filter((room) =>
      `${room.name} ${room.description}`.toLowerCase().includes(normalized),
    )
  }, [query, rooms])

  const createRoom = async (values: CreateRoomValues) => {
    setCreating(true)
    try {
      const room = await endpoints.createRoom(values)
      message.success('房间已创建')
      setCreateOpen(false)
      form.resetFields()
      navigate(`/chat/${room.id}`)
    } catch (error) {
      message.error(errorMessage(error, '创建房间失败'))
    } finally {
      setCreating(false)
    }
  }

  const enterRoom = async (room: Room) => {
    if (room.membership_status === 'active' || room.join_policy === 'open') {
      navigate(`/chat/${room.id}`)
      return
    }
    setJoiningId(room.id)
    try {
      const membership = await endpoints.requestJoin(room.id)
      if (membership.status === 'active') navigate(`/chat/${room.id}`)
      else {
        message.success('加入申请已提交')
        setRooms((current) =>
          current.map((item) =>
            item.id === room.id ? { ...item, membership_status: 'pending' } : item,
          ),
        )
      }
    } catch (error) {
      message.error(errorMessage(error, '无法加入房间'))
    } finally {
      setJoiningId('')
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col bg-white">
      <PageHeader
        title="发现房间"
        description="公开房间与已加入的私密房间"
        actions={
          <>
            <Button icon={<ReloadOutlined />} aria-label="刷新" onClick={() => { setLoading(true); void load() }} />
            <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateOpen(true)}>
              <span className="hidden sm:inline">创建房间</span>
            </Button>
          </>
        }
      />
      <div className="border-b border-[#edf0ef] px-5 py-4 sm:px-7">
        <Input
          allowClear
          prefix={<SearchOutlined />}
          placeholder="按名称或简介筛选"
          className="max-w-xl"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-7">
        {loading ? (
          <div className="py-24 text-center"><Spin /></div>
        ) : filtered.length === 0 ? (
          <Empty description="没有找到房间" />
        ) : (
          <List
            className="mx-auto max-w-4xl"
            dataSource={filtered}
            renderItem={(room) => (
              <List.Item
                className="!items-center"
                actions={[
                  <Button
                    key="enter"
                    type={room.membership_status === 'active' ? 'default' : 'primary'}
                    icon={<ArrowRightOutlined />}
                    loading={joiningId === room.id}
                    disabled={room.membership_status === 'pending'}
                    onClick={() => void enterRoom(room)}
                  >
                    {room.membership_status === 'pending'
                      ? '等待审核'
                      : room.membership_status === 'active'
                        ? '进入'
                        : room.join_policy === 'approval'
                          ? '申请加入'
                          : '加入'}
                  </Button>,
                ]}
              >
                <List.Item.Meta
                  avatar={<UserAvatar emoji={room.avatar_emoji} name={room.name} size={48} />}
                  title={
                    <span className="flex items-center gap-2">
                      <span>{room.name}</span>
                      {room.has_password && <LockOutlined className="text-xs text-[#9a6a25]" />}
                      {room.membership_role && <Tag color="green">{roleLabel(room.membership_role)}</Tag>}
                    </span>
                  }
                  description={
                    <span className="block max-w-2xl">
                      <span className="line-clamp-2">{room.description || '暂无简介'}</span>
                      <span className="mt-1 flex items-center gap-1 text-xs text-[#89958f]">
                        <TeamOutlined /> {room.join_policy === 'approval' ? '审核加入' : '自由加入'}
                      </span>
                    </span>
                  }
                />
              </List.Item>
            )}
          />
        )}
      </div>

      <Modal
        title="创建房间"
        open={createOpen}
        okText="创建"
        cancelText="取消"
        confirmLoading={creating}
        onCancel={() => setCreateOpen(false)}
        onOk={() => form.submit()}
      >
        <Form<CreateRoomValues>
          form={form}
          layout="vertical"
          initialValues={{ join_policy: 'open' }}
          onFinish={createRoom}
        >
          <div className="grid grid-cols-[1fr_88px] gap-3">
            <Form.Item label="房间名称" name="name" rules={[{ required: true }, { max: 80 }]}>
              <Input placeholder="例如：产品讨论" />
            </Form.Item>
            <Form.Item label="图标" name="avatar_emoji" rules={[{ max: 8 }]}>
              <Input placeholder="💬" />
            </Form.Item>
          </div>
          <Form.Item label="简介" name="description" rules={[{ max: 300 }]}>
            <Input.TextArea autoSize={{ minRows: 2, maxRows: 4 }} placeholder="这个房间讨论什么" />
          </Form.Item>
          <Form.Item label="加入方式" name="join_policy">
            <Radio.Group optionType="button" buttonStyle="solid">
              <Radio.Button value="open">自由加入</Radio.Button>
              <Radio.Button value="approval">需要审核</Radio.Button>
            </Radio.Group>
          </Form.Item>
          <Form.Item label="房间密码" name="password" extra="留空即为公开房间">
            <Input.Password maxLength={256} placeholder="可选" />
          </Form.Item>
        </Form>
      </Modal>
    </section>
  )
}

function roleLabel(role: string) {
  return { owner: '群主', admin: '管理员', member: '成员' }[role] ?? role
}
