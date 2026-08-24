import {
  CheckOutlined,
  CloseOutlined,
  MessageOutlined,
  SearchOutlined,
  StopOutlined,
  UserAddOutlined,
  UserDeleteOutlined,
} from '@ant-design/icons'
import { App, Button, Empty, Input, List, Popconfirm, Segmented, Spin, Tag } from 'antd'
import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { PageHeader } from '../components/PageHeader'
import { UserAvatar } from '../components/UserAvatar'
import { endpoints, errorMessage } from '../lib/api'
import { displayName } from '../lib/format'
import type { FriendRequest, SocialUser } from '../types'

type ContactTab = 'friends' | 'requests' | 'search' | 'blocks'

export function ContactsPage() {
  const { message } = App.useApp()
  const navigate = useNavigate()
  const [tab, setTab] = useState<ContactTab>('friends')
  const [friends, setFriends] = useState<SocialUser[]>([])
  const [incoming, setIncoming] = useState<FriendRequest[]>([])
  const [outgoing, setOutgoing] = useState<FriendRequest[]>([])
  const [blocks, setBlocks] = useState<SocialUser[]>([])
  const [results, setResults] = useState<SocialUser[]>([])
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(true)
  const [acting, setActing] = useState('')

  const load = useCallback(async () => {
    try {
      const [friendsData, incomingData, outgoingData, blocksData] = await Promise.all([
        endpoints.friends(),
        endpoints.requests('incoming'),
        endpoints.requests('outgoing'),
        endpoints.blocks(),
      ])
      setFriends(friendsData)
      setIncoming(incomingData)
      setOutgoing(outgoingData)
      setBlocks(blocksData)
    } catch (error) {
      message.error(errorMessage(error, '无法加载联系人'))
    } finally {
      setLoading(false)
    }
  }, [message])

  useEffect(() => {
    let active = true
    void Promise.all([
      endpoints.friends(),
      endpoints.requests('incoming'),
      endpoints.requests('outgoing'),
      endpoints.blocks(),
    ])
      .then(([friendsData, incomingData, outgoingData, blocksData]) => {
        if (!active) return
        setFriends(friendsData)
        setIncoming(incomingData)
        setOutgoing(outgoingData)
        setBlocks(blocksData)
      })
      .catch((error) => message.error(errorMessage(error, '无法加载联系人')))
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [message])

  const run = async (key: string, action: () => Promise<unknown>, success: string) => {
    setActing(key)
    try {
      await action()
      message.success(success)
      await load()
      if (tab === 'search' && query.trim()) setResults(await endpoints.searchUsers(query.trim()))
    } catch (error) {
      message.error(errorMessage(error))
    } finally {
      setActing('')
    }
  }

  const search = async () => {
    if (!query.trim()) return
    setLoading(true)
    try {
      setResults(await endpoints.searchUsers(query.trim()))
    } catch (error) {
      message.error(errorMessage(error, '搜索失败'))
    } finally {
      setLoading(false)
    }
  }

  const startChat = async (userId: string) => {
    setActing(userId)
    try {
      const conversation = await endpoints.startDirect(userId)
      navigate(`/chat/${conversation.room_id}`)
    } catch (error) {
      message.error(errorMessage(error, '无法发起私聊'))
    } finally {
      setActing('')
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col bg-white">
      <PageHeader title="联系人" description="好友、请求与隐私关系" />
      <div className="border-b border-[#e6ebe9] px-5 py-3 sm:px-7">
        <Segmented
          value={tab}
          onChange={(value) => setTab(value as ContactTab)}
          options={[
            { label: `好友 ${friends.length}`, value: 'friends' },
            { label: `请求 ${incoming.length}`, value: 'requests' },
            { label: '查找', value: 'search' },
            { label: '已屏蔽', value: 'blocks' },
          ]}
        />
      </div>
      {tab === 'search' && (
        <div className="border-b border-[#edf0ef] px-5 py-4 sm:px-7">
          <Input.Search
            className="max-w-xl"
            prefix={<SearchOutlined />}
            placeholder="输入用户名或昵称"
            enterButton="搜索"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onSearch={() => void search()}
          />
        </div>
      )}
      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto px-5 py-3 sm:px-7">
        {loading ? (
          <div className="py-24 text-center"><Spin /></div>
        ) : (
          <div className="mx-auto max-w-3xl">
            {tab === 'friends' && (
              <UserList
                users={friends}
                empty="还没有好友"
                actions={(user) => [
                  <Button key="chat" icon={<MessageOutlined />} loading={acting === user.id} onClick={() => void startChat(user.id)}>
                    私聊
                  </Button>,
                  <Popconfirm
                    key="delete"
                    title="删除好友？"
                    description="对应的私聊也将停止使用。"
                    okText="删除"
                    cancelText="取消"
                    onConfirm={() => void run(user.id, () => endpoints.deleteFriend(user.id), '好友已删除')}
                  >
                    <Button danger type="text" icon={<UserDeleteOutlined />} aria-label="删除好友" />
                  </Popconfirm>,
                ]}
              />
            )}
            {tab === 'requests' && (
              <>
                <SectionLabel>收到的请求</SectionLabel>
                <RequestList
                  requests={incoming}
                  empty="暂无收到的请求"
                  actions={(request) => [
                    <Button
                      key="accept"
                      type="primary"
                      icon={<CheckOutlined />}
                      loading={acting === request.user.id}
                      onClick={() =>
                        void run(
                          request.user.id,
                          () => endpoints.updateRequest(request.user.id, 'accept'),
                          '已添加为好友',
                        )
                      }
                    >
                      接受
                    </Button>,
                    <Button
                      key="reject"
                      icon={<CloseOutlined />}
                      onClick={() =>
                        void run(
                          request.user.id,
                          () => endpoints.updateRequest(request.user.id, 'reject'),
                          '已拒绝请求',
                        )
                      }
                    >
                      拒绝
                    </Button>,
                  ]}
                />
                <SectionLabel>发出的请求</SectionLabel>
                <RequestList
                  requests={outgoing}
                  empty="暂无发出的请求"
                  actions={(request) => [
                    <Button
                      key="cancel"
                      onClick={() =>
                        void run(request.user.id, () => endpoints.cancelRequest(request.user.id), '请求已取消')
                      }
                    >
                      取消请求
                    </Button>,
                  ]}
                />
              </>
            )}
            {tab === 'search' && (
              <UserList
                users={results}
                empty={query ? '没有找到用户' : '输入关键词查找用户'}
                actions={(user) => searchActions(user, acting, run, startChat)}
              />
            )}
            {tab === 'blocks' && (
              <UserList
                users={blocks}
                empty="没有已屏蔽的用户"
                actions={(user) => [
                  <Button
                    key="unblock"
                    onClick={() => void run(user.id, () => endpoints.unblock(user.id), '已取消屏蔽')}
                  >
                    取消屏蔽
                  </Button>,
                ]}
              />
            )}
          </div>
        )}
      </div>
    </section>
  )
}

function UserList({ users, empty, actions }: { users: SocialUser[]; empty: string; actions: (user: SocialUser) => ReactNode[] }) {
  if (!users.length) return <Empty className="mt-20" description={empty} />
  return (
    <List
      dataSource={users}
      renderItem={(user) => (
        <List.Item actions={actions(user)}>
          <List.Item.Meta
            avatar={<UserAvatar emoji={user.avatar_emoji} name={user.username} size={44} />}
            title={<span>{displayName(user)} <small className="ml-1 font-normal text-[#839089]">@{user.username}</small></span>}
            description={user.signature || '这个人很安静'}
          />
        </List.Item>
      )}
    />
  )
}

function RequestList({ requests, empty, actions }: { requests: FriendRequest[]; empty: string; actions: (request: FriendRequest) => ReactNode[] }) {
  if (!requests.length) return <p className="py-5 text-sm text-[#7d8983]">{empty}</p>
  return (
    <List
      dataSource={requests}
      renderItem={(request) => (
        <List.Item actions={actions(request)}>
          <List.Item.Meta
            avatar={<UserAvatar emoji={request.user.avatar_emoji} name={request.user.username} size={42} />}
            title={displayName(request.user)}
            description={`@${request.user.username}`}
          />
        </List.Item>
      )}
    />
  )
}

function SectionLabel({ children }: { children: ReactNode }) {
  return <h2 className="mt-5 mb-0 text-xs font-semibold text-[#78857f]">{children}</h2>
}

function searchActions(
  user: SocialUser,
  acting: string,
  run: (key: string, action: () => Promise<unknown>, success: string) => Promise<void>,
  startChat: (userId: string) => Promise<void>,
) {
  if (user.relationship === 'friend') {
    return [
      <Button key="chat" icon={<MessageOutlined />} loading={acting === user.id} onClick={() => void startChat(user.id)}>私聊</Button>,
      <Button key="block" danger type="text" icon={<StopOutlined />} onClick={() => void run(user.id, () => endpoints.block(user.id), '已屏蔽用户')} aria-label="屏蔽用户" />,
    ]
  }
  if (user.relationship === 'outgoing') return [<Tag key="pending">已发送请求</Tag>]
  if (user.relationship === 'incoming') return [<Tag key="incoming" color="blue">等待你处理</Tag>]
  if (user.relationship === 'blocked') return [<Tag key="blocked">已屏蔽</Tag>]
  return [
    <Button key="add" type="primary" icon={<UserAddOutlined />} loading={acting === user.id} onClick={() => void run(user.id, () => endpoints.addFriend(user.id), '好友请求已发送')}>添加好友</Button>,
    <Button key="block" danger type="text" icon={<StopOutlined />} onClick={() => void run(user.id, () => endpoints.block(user.id), '已屏蔽用户')} aria-label="屏蔽用户" />,
  ]
}
