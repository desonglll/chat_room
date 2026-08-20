import type { BroadcastMessage, ReadReceipt, RoomMember } from './types'

export type ServerMessage =
  | {
      type: 'auth_ok'
      room_name: string
      members?: RoomMember[]
      participants?: RoomMember[]
      read_receipts?: ReadReceipt[]
    }
  | { type: 'auth_fail'; reason: string }
  | { type: 'history_complete' }
  | BroadcastMessage
  | { type: 'read_receipt'; user_id: string; username: string; message_id: string }
  | { type: 'message_recalled'; message_id: string; recalled_at: string }
  | { type: 'message_edited'; message_id: string; content: string; edited_at: string }
  | { type: 'reaction_changed'; message_id: string; emoji: string; user_id: string; active: boolean }
  | { type: 'typing'; user_id?: string; username?: string; content: string }
  | { type: 'presence'; members: RoomMember[]; participants: RoomMember[] }
  | { type: 'system'; content: string; members?: RoomMember[]; participants?: RoomMember[] }

export const AUTH_ERRORS: Record<string, string> = {
  'wrong password': '房间密码错误',
  'room not found': '聊天室不存在',
  'authentication timeout': '认证超时，请重试',
  'login required': '请重新登录',
  'authentication unavailable': '暂时无法验证登录状态',
  'password too long': '房间密码过长',
  'membership required': '请先申请加入聊天室',
  'membership pending': '加入申请正在等待管理员审核',
  'invalid json': '认证请求无效',
}

export function readableSystemMessage(content: string): string {
  const joined = content.match(/^(.*) joined the room$/)
  if (joined) return `${joined[1]} 加入了聊天室`
  const left = content.match(/^(.*) left the room$/)
  if (left) return `${left[1]} 离开了聊天室`
  const renamed = content.match(/^room renamed to (.*)$/)
  if (renamed) return `聊天室已重命名为 ${renamed[1]}`
  if (content === 'room deleted') return '聊天室已被删除'
  if (content === 'room password changed') return '聊天室密码已更改，请重新加入'
  if (content === 'message history is temporarily unavailable') return '暂时无法读取历史消息'
  const failed = content.match(/^message from (.*) was not saved or broadcast$/)
  if (failed) return `${failed[1]} 的消息保存失败`
  return content
}
