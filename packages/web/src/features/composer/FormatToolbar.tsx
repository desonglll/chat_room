/**
 * The selection formatting toolbar: appears above the input while text is selected
 * (a textarea exposes no selection rectangle, so it anchors to the field, not the
 * words). Buttons run the same `applyFormat` transforms as the keyboard shortcuts; the
 * link button swaps the row for a URL field. `mousedown` + preventDefault on every
 * control keeps the textarea's selection alive while clicking.
 */
import { useState, type MouseEvent } from 'react'
import type { FormatKind } from '@tg/core'
import { t } from '../../i18n/index'

export interface FormatToolbarProps {
  onFormat(kind: FormatKind, url?: string): void
}

const BUTTONS: ReadonlyArray<{ kind: Exclude<FormatKind, 'link'>; label: string; glyph: string; hint: string }> = [
  {
    kind: 'bold',
    get label() {
      return t('w.composer.67c6b7')
    },
    glyph: 'B',
    hint: 'Ctrl+B',
  },
  {
    kind: 'italic',
    get label() {
      return t('w.composer.af5a2c')
    },
    glyph: 'I',
    hint: 'Ctrl+I',
  },
  {
    kind: 'underline',
    get label() {
      return t('w.composer.9bc18a')
    },
    glyph: 'U',
    hint: 'Ctrl+U',
  },
  {
    kind: 'strike',
    get label() {
      return t('w.composer.c85de9')
    },
    glyph: 'S',
    hint: 'Ctrl+Shift+X',
  },
  {
    kind: 'code',
    get label() {
      return t('w.composer.c49fc6')
    },
    glyph: '</>',
    hint: 'Ctrl+Shift+M',
  },
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
        aria-label={t('w.composer.010ace')}
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
          aria-label={t('w.composer.7223a5')}
          autoFocus
          onChange={(event) => setUrl(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.stopPropagation()
              setLinkMode(false)
            }
          }}
        />
        <button type="submit" className="tg-compose__format-button" aria-label={t('w.composer.f526c8')}>
          ✓
        </button>
      </form>
    )
  }

  return (
    <div className="tg-compose__format" role="toolbar" aria-label={t('w.composer.00986a')}>
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
        aria-label={t('w.composer.715022')}
        title={t('w.composer.7ba5c8')}
        onMouseDown={keepSelection}
        onClick={() => setLinkMode(true)}
      >
        🔗
      </button>
    </div>
  )
}
