/** User-facing copy for a failed round video recording or upload (TG-402). */
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import { RecorderError } from '../voice/voiceRecorder'

export function videoNoteErrorText(error: unknown): string {
  if (error instanceof RecorderError) {
    if (error.reason === 'unsupported') return '此浏览器不支持录制视频消息'
    if (error.reason === 'permission') return '无法使用摄像头：请在浏览器设置中允许访问'
    return '摄像头不可用'
  }
  if (error instanceof ApiError) {
    if (error.serverMessage === VOICE_RESTRICTED) return '对方设置了不接收你的语音和视频消息'
    if (error.status === 403) return '没有在此会话发送消息的权限'
    if (error.status === 413) return '视频消息过大'
  }
  return '视频消息发送失败，请重试'
}
