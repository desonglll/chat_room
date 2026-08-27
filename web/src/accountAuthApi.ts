import { request } from './api'
import type { AuthSession } from './types'

async function authenticate(
  endpoint: 'register' | 'login',
  username: string,
  password: string,
  inviteToken?: string,
): Promise<AuthSession> {
  const response = await request(`/api/users/${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password, invite_token: inviteToken || undefined }),
  })
  if (response.status === 400) throw new Error('用户名不符合要求，密码至少需要 8 个字符')
  if (response.status === 401) throw new Error('用户名或密码错误')
  if (response.status === 403) throw new Error('注册不可用或邀请码无效')
  if (response.status === 409) throw new Error('用户名已被注册')
  if (!response.ok) throw new Error(`${endpoint === 'register' ? '注册' : '登录'}失败：${response.status}`)
  return response.json() as Promise<AuthSession>
}

export function registerUser(username: string, password: string, inviteToken = ''): Promise<AuthSession> {
  return authenticate('register', username, password, inviteToken)
}

export function loginUser(username: string, password: string): Promise<AuthSession> {
  return authenticate('login', username, password)
}
