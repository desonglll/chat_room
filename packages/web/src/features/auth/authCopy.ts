/**
 * Human copy for the auth flow. `ApiError` deliberately carries no UI copy
 * (TG-011 decision); statuses map to Chinese here, at the feature boundary, and
 * server `error` strings are never string-matched so backend copy can change freely.
 */
import { ApiError } from '@tg/core'
import type { AuthMode } from '../../app/session'

export function authErrorCopy(error: unknown, mode: AuthMode): string {
  if (error instanceof ApiError) {
    switch (error.status) {
      case 400:
        return '用户名或密码不符合要求：用户名不超过 48 个字符，密码 8–256 位'
      case 401:
        return '用户名或密码不正确'
      case 403:
        return mode === 'register' ? '当前部署仅限邀请注册，请检查邀请码' : '没有权限执行此操作'
      case 409:
        return '这个用户名已被使用'
      case 429:
        return '尝试次数过多，请稍后再试'
      default:
        return `请求失败（${error.status}），请稍后再试`
    }
  }
  return '无法连接服务器，请检查网络后重试'
}
