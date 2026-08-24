import {
  CloseOutlined,
  PaperClipOutlined,
  SendOutlined,
  SmileOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons'
import { Alert, App, Button, Input, Popover, Tooltip, Upload } from 'antd'
import { useEffect, useRef, useState } from 'react'
import type { UploadFile } from 'antd'
import { endpoints, errorMessage } from '../../lib/api'
import type { AiSuggestions, StoredMessage } from '../../types'

const EMOJIS = ['😀', '😄', '😂', '🥹', '😊', '😍', '🤔', '👍', '👏', '🎉', '❤️', '✨']

interface MessageComposerProps {
  roomId: string
  roomPassword?: string
  replyTo: StoredMessage | null
  disabled: boolean
  aiEnabled: boolean
  onClearReply: () => void
  onSendText: (content: string, replyId?: string) => boolean
  onTyping: (content: string) => void
  onUploaded: (message: StoredMessage) => void
}

export function MessageComposer({
  roomId,
  roomPassword,
  replyTo,
  disabled,
  aiEnabled,
  onClearReply,
  onSendText,
  onTyping,
  onUploaded,
}: MessageComposerProps) {
  const { message } = App.useApp()
  const [draft, setDraft] = useState('')
  const [file, setFile] = useState<UploadFile | null>(null)
  const [sensitive, setSensitive] = useState(false)
  const [sending, setSending] = useState(false)
  const [assistant, setAssistant] = useState<AiSuggestions | null>(null)
  const [assistantError, setAssistantError] = useState('')
  const [assistantLoading, setAssistantLoading] = useState(false)
  const typingTimer = useRef<number | undefined>(undefined)

  useEffect(() => () => window.clearTimeout(typingTimer.current), [])

  const updateDraft = (value: string) => {
    setDraft(value)
    onTyping(value)
    window.clearTimeout(typingTimer.current)
    typingTimer.current = window.setTimeout(() => onTyping(''), 1_200)
  }

  const submit = async () => {
    const content = draft.trim()
    if ((!content && !file) || disabled) return
    setSending(true)
    try {
      if (file?.originFileObj) {
        const form = new FormData()
        form.append('file', file.originFileObj)
        if (content) form.append('content', content)
        if (replyTo) form.append('reply_to', replyTo.id)
        form.append('is_sensitive', String(sensitive))
        onUploaded(await endpoints.upload(roomId, form, roomPassword))
      } else if (!onSendText(content, replyTo?.id)) {
        return
      }
      setDraft('')
      setFile(null)
      setSensitive(false)
      setAssistant(null)
      onClearReply()
      onTyping('')
    } catch (error) {
      message.error(errorMessage(error, '消息发送失败'))
    } finally {
      setSending(false)
    }
  }

  const requestSuggestions = async () => {
    setAssistantLoading(true)
    setAssistantError('')
    try {
      setAssistant(await endpoints.aiSuggestions(roomId))
    } catch (error) {
      setAssistantError(errorMessage(error, '暂时无法生成建议'))
    } finally {
      setAssistantLoading(false)
    }
  }

  return (
    <footer className="border-t border-[#dfe5e2] bg-white px-3 py-3 sm:px-5">
      <div className="mx-auto max-w-3xl">
        {replyTo && (
          <div className="mb-2 flex items-center gap-3 border-l-2 border-[#087f5b] bg-[#f0f6f3] px-3 py-2 text-xs">
            <span className="min-w-0 flex-1">
              <strong className="block text-[#087f5b]">回复 {replyTo.sender}</strong>
              <span className="block truncate text-[#66736d]">
                {replyTo.content || replyTo.attachment?.file_name || '附件'}
              </span>
            </span>
            <Button type="text" size="small" icon={<CloseOutlined />} onClick={onClearReply} aria-label="取消回复" />
          </div>
        )}
        {file && (
          <div className="mb-2 flex items-center gap-2 rounded-md border border-[#dce3e0] bg-[#f8faf9] px-3 py-2 text-xs">
            <PaperClipOutlined />
            <span className="min-w-0 flex-1 truncate">{file.name}</span>
            <label className="flex cursor-pointer items-center gap-1.5 text-[#7a4a1b]">
              <input
                type="checkbox"
                checked={sensitive}
                onChange={(event) => setSensitive(event.target.checked)}
              />
              敏感内容
            </label>
            <Button type="text" size="small" icon={<CloseOutlined />} onClick={() => setFile(null)} aria-label="移除附件" />
          </div>
        )}
        {assistant && (
          <div className="mb-2 border-l-2 border-[#2563eb] bg-[#f1f5ff] px-3 py-2 text-xs">
            <p className="m-0 mb-2 text-[#42526e]">{assistant.summary}</p>
            <div className="flex flex-wrap gap-1.5">
              {assistant.suggestions.map((reply) => (
                <button
                  key={reply}
                  type="button"
                  className="rounded-full border border-[#b9c9f2] bg-white px-2.5 py-1 text-[#244ca0]"
                  onClick={() => setDraft(reply)}
                >
                  {reply}
                </button>
              ))}
            </div>
          </div>
        )}
        {assistantError && (
          <Alert className="mb-2" type="warning" showIcon message={assistantError} closable onClose={() => setAssistantError('')} />
        )}
        <div className="flex items-end gap-1 rounded-lg border border-[#d7dfdb] bg-white p-1.5 shadow-sm focus-within:border-[#58a98d]">
          <Upload
            accept="image/*,video/*,audio/*,.pdf,.doc,.docx,.xls,.xlsx,.zip,.txt"
            beforeUpload={(nextFile) => {
              setFile(nextFile)
              return false
            }}
            fileList={[]}
            maxCount={1}
          >
            <Tooltip title="添加附件">
              <Button type="text" icon={<PaperClipOutlined />} aria-label="添加附件" disabled={disabled} />
            </Tooltip>
          </Upload>
          <Popover
            trigger="click"
            content={
              <div className="grid grid-cols-6 gap-1">
                {EMOJIS.map((emoji) => (
                  <Button key={emoji} type="text" size="small" onClick={() => setDraft((value) => value + emoji)}>
                    {emoji}
                  </Button>
                ))}
              </div>
            }
          >
            <Tooltip title="表情">
              <Button type="text" icon={<SmileOutlined />} aria-label="表情" disabled={disabled} />
            </Tooltip>
          </Popover>
          {aiEnabled && (
            <Tooltip title="对话建议">
              <Button
                type="text"
                icon={<ThunderboltOutlined />}
                aria-label="对话建议"
                loading={assistantLoading}
                disabled={disabled}
                onClick={requestSuggestions}
              />
            </Tooltip>
          )}
          <Input.TextArea
            className="!border-0 !shadow-none"
            autoSize={{ minRows: 1, maxRows: 6 }}
            value={draft}
            placeholder={disabled ? '连接后即可发送消息' : '输入消息'}
            disabled={disabled}
            maxLength={4096}
            onChange={(event) => updateDraft(event.target.value)}
            onPressEnter={(event) => {
              if (!event.shiftKey) {
                event.preventDefault()
                void submit()
              }
            }}
          />
          <Button
            type="primary"
            icon={<SendOutlined />}
            aria-label="发送"
            loading={sending}
            disabled={disabled || (!draft.trim() && !file)}
            onClick={() => void submit()}
          />
        </div>
      </div>
    </footer>
  )
}
