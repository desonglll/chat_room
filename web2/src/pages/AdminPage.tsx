import {
  CloudServerOutlined,
  DatabaseOutlined,
  DeleteOutlined,
  MessageOutlined,
  ReloadOutlined,
  TeamOutlined,
} from '@ant-design/icons'
import { App, Button, Descriptions, Empty, Result, Spin, Statistic, Switch, Table, Tag } from 'antd'
import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { PageHeader } from '../components/PageHeader'
import { endpoints, errorMessage } from '../lib/api'
import { formatBytes, formatDuration, formatTime } from '../lib/format'
import type { AdminOverview } from '../types'

export function AdminPage() {
  const { message, modal } = App.useApp()
  const [overview, setOverview] = useState<AdminOverview | null>(null)
  const [loading, setLoading] = useState(true)
  const [forbidden, setForbidden] = useState(false)
  const [lockSaving, setLockSaving] = useState(false)

  const load = useCallback(async () => {
    try {
      setOverview(await endpoints.adminOverview())
      setForbidden(false)
    } catch (error: unknown) {
      const status = (error as { response?: { status?: number } }).response?.status
      if (status === 403) setForbidden(true)
      else message.error(errorMessage(error, '无法加载管理数据'))
    } finally {
      setLoading(false)
    }
  }, [message])

  useEffect(() => {
    let active = true
    void endpoints
      .adminOverview()
      .then((data) => {
        if (!active) return
        setOverview(data)
        setForbidden(false)
      })
      .catch((error: unknown) => {
        if (!active) return
        const status = (error as { response?: { status?: number } }).response?.status
        if (status === 403) setForbidden(true)
        else message.error(errorMessage(error, '无法加载管理数据'))
      })
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [message])

  const toggleLock = async (locked: boolean) => {
    setLockSaving(true)
    try {
      await endpoints.setChatLock(locked)
      setOverview((current) => (current ? { ...current, chat_rooms_locked: locked } : current))
      message.success(locked ? '所有聊天房间已锁定' : '聊天房间已恢复')
    } catch (error) {
      message.error(errorMessage(error, '无法更新聊天锁定状态'))
    } finally {
      setLockSaving(false)
    }
  }

  const purge = () => {
    modal.confirm({
      title: '执行保留数据清理？',
      content: '将清理已超过保留期的附件对象和软删除房间。',
      okText: '开始清理',
      cancelText: '取消',
      okButtonProps: { danger: true },
      onOk: async () => {
        const result = await endpoints.purge()
        message.success(
          `已删除 ${result.attachment_objects_deleted} 个附件对象和 ${result.rooms_deleted} 个房间`,
        )
        await load()
      },
    })
  }

  if (forbidden) {
    return (
      <Result
        status="403"
        title="无管理权限"
        subTitle="当前账号未配置为系统管理员。"
      />
    )
  }

  return (
    <section className="flex h-full min-h-0 flex-col bg-[#f6f8f7]">
      <PageHeader
        title="系统管理"
        description={overview ? `数据生成于 ${new Date(overview.generated_at).toLocaleString('zh-CN')}` : '运行状态与保留策略'}
        actions={<Button icon={<ReloadOutlined />} onClick={() => { setLoading(true); void load() }}>刷新</Button>}
      />
      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto">
        {loading && !overview ? (
          <div className="py-24 text-center"><Spin /></div>
        ) : overview ? (
          <div className="mx-auto max-w-6xl px-5 py-6 sm:px-7">
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
              <Metric icon={<TeamOutlined />} title="在线用户" value={overview.online_users} />
              <Metric icon={<MessageOutlined />} title="实时连接" value={overview.websocket_connections} />
              <Metric icon={<DatabaseOutlined />} title="消息总数" value={overview.totals.messages} />
              <Metric icon={<CloudServerOutlined />} title="物理存储" value={formatBytes(overview.storage.physical_bytes)} />
            </div>

            <section className="mt-6 border-y border-[#dfe5e2] bg-white px-5 py-5">
              <div className="flex flex-wrap items-center justify-between gap-4">
                <div>
                  <h2 className="m-0 text-base font-semibold">聊天总开关</h2>
                  <p className="mt-1 mb-0 text-sm text-[#6f7c76]">锁定后会断开所有房间连接，并阻止新建房间。</p>
                </div>
                <Switch
                  checked={overview.chat_rooms_locked}
                  loading={lockSaving}
                  checkedChildren="已锁定"
                  unCheckedChildren="运行中"
                  onChange={(checked) => void toggleLock(checked)}
                />
              </div>
            </section>

            <section className="mt-6 bg-white px-5 py-5">
              <h2 className="mt-0 mb-4 text-base font-semibold">活跃房间</h2>
              <Table
                rowKey="id"
                size="middle"
                pagination={false}
                dataSource={overview.top_rooms}
                scroll={{ x: 560 }}
                columns={[
                  { title: '房间', dataIndex: 'name', ellipsis: true },
                  { title: '消息', dataIndex: 'messages', width: 100 },
                  { title: '成员', dataIndex: 'active_members', width: 100 },
                  {
                    title: '最近消息',
                    dataIndex: 'last_message_at',
                    width: 130,
                    render: (value: string | null) => (value ? formatTime(value) : '暂无'),
                  },
                ]}
              />
            </section>

            <section className="mt-6 grid gap-5 bg-white px-5 py-5 lg:grid-cols-2">
              <div>
                <h2 className="mt-0 mb-4 text-base font-semibold">运行信息</h2>
                <Descriptions size="small" column={1}>
                  <Descriptions.Item label="运行时长">{formatDuration(overview.runtime.uptime_seconds)}</Descriptions.Item>
                  <Descriptions.Item label="请求总数">{overview.runtime.requests.toLocaleString()}</Descriptions.Item>
                  <Descriptions.Item label="平均延迟">{overview.runtime.average_latency_ms.toFixed(1)} ms</Descriptions.Item>
                  <Descriptions.Item label="最高延迟">{overview.runtime.max_latency_ms.toFixed(1)} ms</Descriptions.Item>
                  <Descriptions.Item label="数据库"><Tag>{overview.database_backend}</Tag></Descriptions.Item>
                  <Descriptions.Item label="附件后端"><Tag>{overview.attachment_backend}</Tag></Descriptions.Item>
                </Descriptions>
              </div>
              <div>
                <h2 className="mt-0 mb-4 text-base font-semibold">保留数据</h2>
                <Descriptions size="small" column={1}>
                  <Descriptions.Item label="孤立附件">{overview.storage.orphaned_attachments}</Descriptions.Item>
                  <Descriptions.Item label="孤立数据">{formatBytes(overview.storage.orphaned_bytes)}</Descriptions.Item>
                  <Descriptions.Item label="待完成上传">{overview.totals.pending_uploads}</Descriptions.Item>
                  <Descriptions.Item label="附件保留">{overview.orphan_retention_hours} 小时</Descriptions.Item>
                  <Descriptions.Item label="房间保留">{overview.deleted_room_retention_days} 天</Descriptions.Item>
                </Descriptions>
                <Button danger icon={<DeleteOutlined />} className="mt-4" onClick={purge}>立即清理</Button>
              </div>
            </section>
          </div>
        ) : (
          <Empty className="mt-20" description="暂无管理数据" />
        )}
      </div>
    </section>
  )
}

function Metric({ icon, title, value }: { icon: ReactNode; title: string; value: number | string }) {
  return (
    <div className="rounded-md border border-[#e0e5e3] bg-white p-4">
      <div className="mb-3 flex h-8 w-8 items-center justify-center rounded-md bg-[#e7f3ee] text-[#087f5b]">{icon}</div>
      <Statistic title={title} value={value} valueStyle={{ fontSize: 22, fontWeight: 600 }} />
    </div>
  )
}
