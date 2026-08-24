import { LockOutlined, MessageOutlined, UserOutlined } from '@ant-design/icons'
import { Alert, Button, Form, Input, Segmented, Typography } from 'antd'
import { useState } from 'react'
import { Navigate, useLocation, useNavigate } from 'react-router'
import { useAuth } from '../features/auth/auth-context'
import { errorMessage } from '../lib/api'

interface AuthValues {
  username: string
  password: string
}

export function AuthPage() {
  const { session, authenticate } = useAuth()
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()
  const location = useLocation()

  if (session) return <Navigate to="/chat" replace />

  const submit = async (values: AuthValues) => {
    setLoading(true)
    setError('')
    try {
      await authenticate(mode, values.username, values.password)
      const destination = (location.state as { from?: string } | null)?.from ?? '/chat'
      navigate(destination, { replace: true })
    } catch (requestError) {
      setError(
        errorMessage(
          requestError,
          mode === 'login' ? '用户名或密码错误' : '注册失败，用户名可能已被使用',
        ),
      )
    } finally {
      setLoading(false)
    }
  }

  return (
    <main className="auth-backdrop flex min-h-dvh items-center justify-center p-5 md:p-10">
      <section className="grid w-full max-w-[980px] overflow-hidden rounded-lg bg-white shadow-2xl md:grid-cols-[1fr_420px]">
        <div className="hidden min-h-[610px] flex-col justify-between bg-[#eaf5f0] p-12 md:flex">
          <div className="flex items-center gap-3 text-[#15211d]">
            <span className="flex h-11 w-11 items-center justify-center rounded-md bg-[#f1b84b] text-xl font-black">
              栖
            </span>
            <span className="text-sm font-semibold tracking-normal">QIYU CHAT</span>
          </div>
          <div aria-hidden="true" className="relative mx-auto h-72 w-full max-w-sm">
            <div className="absolute left-4 top-10 h-40 w-56 rounded-lg border border-[#bdd4ca] bg-white p-5 shadow-lg">
              <div className="mb-5 flex items-center gap-3">
                <span className="h-9 w-9 rounded-full bg-[#087f5b]" />
                <span className="h-2.5 w-24 rounded bg-[#cbd8d2]" />
              </div>
              <div className="ml-12 h-12 rounded-md bg-[#e8f4ef]" />
              <div className="mt-3 h-9 w-2/3 rounded-md bg-[#f0f2f1]" />
            </div>
            <div className="absolute bottom-4 right-3 h-36 w-52 rounded-lg border border-[#d5ddd9] bg-[#15211d] p-5 shadow-xl">
              <MessageOutlined className="mb-6 text-2xl text-[#f1b84b]" />
              <div className="h-2.5 w-28 rounded bg-white/60" />
              <div className="mt-3 h-2 w-36 rounded bg-white/20" />
            </div>
          </div>
          <div>
            <Typography.Title level={2} className="!mb-2 !text-[30px] !font-semibold !text-[#15211d]">
              对话，在这里安静发生
            </Typography.Title>
            <p className="m-0 text-sm text-[#5c6f67]">栖语 · {new Date().getFullYear()}</p>
          </div>
        </div>

        <div className="flex min-h-[560px] flex-col justify-center px-6 py-10 sm:px-12">
          <div className="mb-9 flex items-center gap-3 md:hidden">
            <span className="flex h-10 w-10 items-center justify-center rounded-md bg-[#f1b84b] font-black text-[#15211d]">
              栖
            </span>
            <strong className="text-lg text-[#15211d]">栖语</strong>
          </div>
          <Typography.Title level={2} className="!mb-2 !text-[26px] !font-semibold">
            {mode === 'login' ? '欢迎回来' : '创建账号'}
          </Typography.Title>
          <Typography.Text type="secondary" className="mb-7 block">
            {mode === 'login' ? '登录后继续你的对话' : '设置用户名和至少 8 位密码'}
          </Typography.Text>

          <Segmented
            block
            className="mb-6"
            value={mode}
            options={[
              { label: '登录', value: 'login' },
              { label: '注册', value: 'register' },
            ]}
            onChange={(value) => {
              setMode(value as 'login' | 'register')
              setError('')
            }}
          />

          {error && <Alert className="mb-5" message={error} type="error" showIcon />}
          <Form<AuthValues> layout="vertical" requiredMark={false} onFinish={submit}>
            <Form.Item
              label="用户名"
              name="username"
              rules={[
                { required: true, message: '请输入用户名' },
                { max: 48, message: '用户名不能超过 48 个字符' },
              ]}
            >
              <Input prefix={<UserOutlined />} autoComplete="username" placeholder="你的用户名" />
            </Form.Item>
            <Form.Item
              label="密码"
              name="password"
              rules={[
                { required: true, message: '请输入密码' },
                { min: 8, message: '密码至少需要 8 位' },
              ]}
            >
              <Input.Password
                prefix={<LockOutlined />}
                autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
                placeholder="至少 8 位"
              />
            </Form.Item>
            <Button type="primary" htmlType="submit" loading={loading} block size="large" className="mt-2">
              {mode === 'login' ? '登录' : '注册并登录'}
            </Button>
          </Form>
        </div>
      </section>
    </main>
  )
}
