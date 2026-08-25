export function assertAiResponse(response: Response): void {
  if (response.status === 400) throw new Error('问题内容无效或问答历史过长')
  if (response.status === 401) throw new Error('登录已过期或聊天室密码错误')
  if (response.status === 403) throw new Error('你已无法访问这个会话')
  if (response.status === 404) throw new Error('会话已不存在')
  if (response.status === 409) throw new Error('这个对话仍在生成上一条回答')
  if (response.status === 429) throw new Error('请求过于频繁，请稍后再试')
  if (response.status === 503) throw new Error('AI 助手当前不可用')
  if (!response.ok) throw new Error(`AI 请求失败：${response.status}`)
}
