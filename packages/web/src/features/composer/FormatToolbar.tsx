/**
 * The selection formatting toolbar: appears above the input while text is selected
 * (a textarea exposes no selection rectangle, so it anchors to the field, not the
 * words). Buttons run the same `applyFormat` transforms as the keyboard shortcuts; the
 * link button swaps the row for a URL field. `mousedown` + preventDefault on every
 * control keeps the textarea's selection alive while clicking.
 */
import { useState, type MouseEvent } from 'react'
import type { FormatKind } from '@tg/core'

export interface FormatToolbarProps {
  onFormat(kind: FormatKind, url?: string): void
}

const BUTTONS: ReadonlyArray<{ kind: Exclude<FormatKind, 'link'>; label: string; glyph: string; hint: string }> = [
  { kind: 'bold', label: '粗体', glyph: 'B', hint: 'Ctrl+B' },
  { kind: 'italic', label: '斜体', glyph: 'I', hint: 'Ctrl+I' },
  { kind: 'underline', label: '下划线', glyph: 'U', hint: 'Ctrl+U' },
  { kind: 'strike', label: '删除线', glyph: 'S', hint: 'Ctrl+Shift+X' },
  { kind: 'code', label: '等宽', glyph: '</>', hint: 'Ctrl+Shift+M' },
]

const keepSelection = (event: MouseEvent) => event.preventDefault()

export function FormatToolbar({ onFormat }: FormatToolbarProps) {
  const [linkMode, setLinkMode] = useState(false)
  const [url, setUrl] = useState('')

  if (linkMode) {
    return (
      <form
        className="tg-compose__format"
        role="toolbar"
        aria-label="插入链接"
        onSubmit={(event) => {
          event.preventDefault()
          onFormat('link', url)
          setLinkMode(false)
          setUrl('')
        }}
      >
        <input
          className="tg-compose__format-url"
          type="url"
          value={url}
          placeholder="https://"
          aria-label="链接地址"
          autoFocus
          onChange={(event) => setUrl(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.stopPropagation()
              setLinkMode(false)
            }
          }}
        />
        <button type="submit" className="tg-compose__format-button" aria-label="确定">
          ✓
        </button>
      </form>
    )
  }

  return (
    <div className="tg-compose__format" role="toolbar" aria-label="文本格式">
      {BUTTONS.map((button) => (
        <button
          key={button.kind}
          type="button"
          className="tg-compose__format-button"
          data-kind={button.kind}
          aria-label={button.label}
          title={`${button.label} (${button.hint})`}
          onMouseDown={keepSelection}
          onClick={() => onFormat(button.kind)}
        >
          {button.glyph}
        </button>
      ))}
      <button
        type="button"
        className="tg-compose__format-button"
        data-kind="link"
        aria-label="链接"
        title="链接 (Ctrl+K)"
        onMouseDown={keepSelection}
        onClick={() => setLinkMode(true)}
      >
        🔗
      </button>
    </div>
  )
}
