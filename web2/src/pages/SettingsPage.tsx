import { DeleteOutlined, LockOutlined, LogoutOutlined, SaveOutlined } from '@ant-design/icons'
import { App, Button, Divider, Form, Input, Modal, Typography } from 'antd'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { PageHeader } from '../components/PageHeader'
import { UserAvatar } from '../components/UserAvatar'
import { useAuth } from '../features/auth/auth-context'
import { endpoints, errorMessage } from '../lib/api'

interface ProfileValues {
  avatar_emoji: string
  display_name: string
  signature: string
  homepage: string
}

interface PasswordValues {
  current_password: string
  new_password: string
  confirm_password: string
}

export function SettingsPage() {
  const { user, updateUser, logout } = useAuth()
  const { message, modal } = App.useApp()
  const navigate = useNavigate()
  const [profileSaving, setProfileSaving] = useState(false)
  const [passwordSaving, setPasswordSaving] = useState(false)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deletePassword, setDeletePassword] = useState('')
  const [deleting, setDeleting] = useState(false)

  if (!user) return null

  const saveProfile = async (values: ProfileValues) => {
    setProfileSaving(true)
    try {
      const updated = await endpoints.updateProfile(values)
      updateUser(updated)
      message.success('个人资料已保存')
    } catch (error) {
      message.error(errorMessage(error, '保存失败'))
    } finally {
      setProfileSaving(false)
    }
  }

  const savePassword = async (values: PasswordValues) => {
    setPasswordSaving(true)
    try {
      await endpoints.changePassword(values.current_password, values.new_password)
      message.success('密码已更新')
    } catch (error) {
      message.error(errorMessage(error, '密码更新失败'))
    } finally {
      setPasswordSaving(false)
    }
  }

  const signOut = () => {
    modal.confirm({
      title: '退出当前账号？',
      okText: '退出',
      cancelText: '取消',
      onOk: async () => {
        await logout()
        navigate('/auth', { replace: true })
      },
    })
  }

  const deleteAccount = async () => {
    setDeleting(true)
    try {
      await endpoints.deleteAccount(deletePassword)
      setDeleteOpen(false)
      await logout()
      navigate('/auth', { replace: true })
    } catch (error) {
      message.error(errorMessage(error, '无法删除账号'))
    } finally {
      setDeleting(false)
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col bg-white">
      <PageHeader title="账号设置" description={`@${user.username}`} />
      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl px-5 py-7 sm:px-8">
          <section className="grid gap-7 sm:grid-cols-[180px_1fr]">
            <div>
              <Typography.Title level={4} className="!mb-1">个人资料</Typography.Title>
              <Typography.Text type="secondary" className="text-sm">这些信息对其他用户可见</Typography.Text>
            </div>
            <div>
              <div className="mb-5 flex items-center gap-3">
                <UserAvatar emoji={user.avatar_emoji} name={user.username} size={54} />
                <div>
                  <strong className="block">{user.display_name || user.username}</strong>
                  <span className="text-xs text-[#7a8781]">@{user.username}</span>
                </div>
              </div>
              <Form<ProfileValues>
                layout="vertical"
                initialValues={user}
                requiredMark={false}
                onFinish={saveProfile}
              >
                <div className="grid gap-x-3 sm:grid-cols-[96px_1fr]">
                  <Form.Item label="头像" name="avatar_emoji" rules={[{ max: 8 }]}>
                    <Input placeholder="🙂" />
                  </Form.Item>
                  <Form.Item label="显示名称" name="display_name" rules={[{ max: 48 }]}>
                    <Input placeholder={user.username} />
                  </Form.Item>
                </div>
                <Form.Item label="个性签名" name="signature" rules={[{ max: 160 }]}>
                  <Input.TextArea autoSize={{ minRows: 2, maxRows: 4 }} showCount maxLength={160} />
                </Form.Item>
                <Form.Item label="个人主页" name="homepage" rules={[{ max: 240 }, { type: 'url', warningOnly: true }]}>
                  <Input placeholder="https://" />
                </Form.Item>
                <Button type="primary" htmlType="submit" icon={<SaveOutlined />} loading={profileSaving}>保存资料</Button>
              </Form>
            </div>
          </section>

          <Divider className="!my-8" />

          <section className="grid gap-7 sm:grid-cols-[180px_1fr]">
            <div>
              <Typography.Title level={4} className="!mb-1">登录安全</Typography.Title>
              <Typography.Text type="secondary" className="text-sm">更新当前账号密码</Typography.Text>
            </div>
            <Form<PasswordValues> layout="vertical" requiredMark={false} onFinish={savePassword}>
              <Form.Item label="当前密码" name="current_password" rules={[{ required: true }]}>
                <Input.Password autoComplete="current-password" />
              </Form.Item>
              <Form.Item label="新密码" name="new_password" rules={[{ required: true }, { min: 8 }]}>
                <Input.Password autoComplete="new-password" />
              </Form.Item>
              <Form.Item
                label="确认新密码"
                name="confirm_password"
                dependencies={['new_password']}
                rules={[
                  { required: true },
                  ({ getFieldValue }) => ({
                    validator(_, value) {
                      return !value || getFieldValue('new_password') === value
                        ? Promise.resolve()
                        : Promise.reject(new Error('两次输入的密码不一致'))
                    },
                  }),
                ]}
              >
                <Input.Password autoComplete="new-password" />
              </Form.Item>
              <Button htmlType="submit" icon={<LockOutlined />} loading={passwordSaving}>更新密码</Button>
            </Form>
          </section>

          <Divider className="!my-8" />

          <section className="grid gap-7 sm:grid-cols-[180px_1fr]">
            <div>
              <Typography.Title level={4} className="!mb-1">会话与账号</Typography.Title>
              <Typography.Text type="secondary" className="text-sm">管理当前登录状态</Typography.Text>
            </div>
            <div className="space-y-3">
              <Button icon={<LogoutOutlined />} onClick={signOut}>退出登录</Button>
              <div className="border-t border-[#ecefee] pt-4">
                <Button danger icon={<DeleteOutlined />} onClick={() => setDeleteOpen(true)}>删除账号</Button>
                <p className="mt-2 mb-0 text-xs text-[#8a504f]">删除后将退出所有房间，且无法恢复。</p>
              </div>
            </div>
          </section>
        </div>
      </div>

      <Modal
        title="删除账号"
        open={deleteOpen}
        okText="永久删除"
        cancelText="取消"
        okButtonProps={{ danger: true, disabled: !deletePassword }}
        confirmLoading={deleting}
        onCancel={() => setDeleteOpen(false)}
        onOk={() => void deleteAccount()}
      >
        <p>请输入当前密码确认删除。此操作无法撤销。</p>
        <Input.Password value={deletePassword} autoComplete="current-password" onChange={(event) => setDeletePassword(event.target.value)} />
      </Modal>
    </section>
  )
}
