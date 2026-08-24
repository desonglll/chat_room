import { EditOutlined, PlusOutlined, SearchOutlined } from '@ant-design/icons'
import { Badge, Button, Input, Tooltip } from 'antd'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { formatTime } from '../../lib/format'
import type { Conversation } from '../../types'
import { UserAvatar } from '../../components/UserAvatar'

export function ConversationList({
  conversations,
  selectedId,
  loading,
}: {
  conversations: Conversation[]
  selectedId?: string
  loading: boolean
}) {
  const [query, setQuery] = useState('')
  const navigate = useNavigate()
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return conversations
    return conversations.filter((conversation) =>
      `${conversation.title} ${conversation.description}`.toLowerCase().includes(normalized),
    )
  }, [conversations, query])

  return (
    <section className="flex h-full min-h-0 w-full flex-col bg-[#f7f9f8] md:w-[330px] md:min-w-[280px] md:max-w-[360px]">
      <header className="border-b border-[#e1e6e4] px-4 pt-5 pb-4">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <p className="m-0 text-xs font-semibold text-[#087f5b]">QIYU</p>
            <h1 className="m-0 mt-0.5 text-xl font-semibold text-[#17201d]">消息</h1>
          </div>
          <Tooltip title="发现或创建房间">
            <Button
              type="text"
              icon={<EditOutlined />}
              aria-label="发现或创建房间"
              onClick={() => navigate('/discover')}
            />
          </Tooltip>
        </div>
        <Input
          allowClear
          prefix={<SearchOutlined className="text-[#82908a]" />}
          placeholder="搜索会话"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </header>

      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto py-2" aria-busy={loading}>
        {!loading && filtered.length === 0 && (
          <div className="flex h-52 flex-col items-center justify-center px-6 text-center text-sm text-[#7a8781]">
            <MessageEmptyIcon />
            <p className="mt-4 mb-3">{query ? '没有匹配的会话' : '还没有会话'}</p>
            {!query && (
              <Button type="link" icon={<PlusOutlined />} onClick={() => navigate('/discover')}>
                找一个房间
              </Button>
            )}
          </div>
        )}
        {filtered.map((conversation) => {
          const selected = selectedId === conversation.room_id
          const preview = conversation.last_message
          return (
            <button
              key={conversation.room_id}
              type="button"
              onClick={() => navigate(`/chat/${conversation.room_id}`)}
              className={`mx-2 flex w-[calc(100%_-_16px)] items-center gap-3 rounded-md px-3 py-3 text-left transition-colors focus-visible:outline-2 focus-visible:outline-[#087f5b] ${
                selected ? 'bg-[#dff1e9]' : 'hover:bg-[#edf1ef]'
              }`}
            >
              <Badge count={conversation.unread_count} size="small" overflowCount={99}>
                <UserAvatar emoji={conversation.avatar_emoji} name={conversation.title} size={46} />
              </Badge>
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline justify-between gap-2">
                  <strong className="truncate text-sm font-semibold text-[#1b2521]">
                    {conversation.title}
                  </strong>
                  <time className="shrink-0 text-[11px] text-[#89958f]">
                    {formatTime(conversation.last_activity_at)}
                  </time>
                </span>
                <span className="mt-1 block truncate text-xs text-[#6e7a75]">
                  {preview?.recalled
                    ? '消息已撤回'
                    : preview?.attachment_file_name
                      ? `[文件] ${preview.attachment_file_name}`
                      : preview?.content || conversation.description || '开始一段对话'}
                </span>
              </span>
            </button>
          )
        })}
      </div>
    </section>
  )
}

function MessageEmptyIcon() {
  return (
    <span className="flex h-12 w-12 items-center justify-center rounded-full bg-[#e3eae7] text-xl text-[#5f7068]">
      <span aria-hidden="true">···</span>
    </span>
  )
}
