/**
 * The emoji picker: `emoji-picker-element`, loaded with `import()` the first time the
 * panel opens so its ~40 KB (gzip) stays out of the entry chunk (bundle budget), and fed
 * the server-hosted Chinese annotations (`/emoji-data-zh.json`, same as the old client).
 *
 * The web component reads its palette only from a `.light`/`.dark` class on itself, so
 * a MutationObserver mirrors TG-009's `data-tg-theme` onto it; the colours themselves
 * are routed through `--tg-*` tokens in composer.css.
 *
 * TG-1301: over plain http on a LAN address the picker's data checksum needs `crypto.subtle`,
 * which the browser withholds outside a secure context; `ensureSubtleDigest` fills that gap.
 */
import { useEffect, useRef, useState } from 'react'
import { t } from '../../i18n/index'
import { ensureSubtleDigest } from './sha1Shim'

export interface EmojiPanelProps {
  onPick(emoji: string): void
}

interface EmojiClickDetail {
  unicode?: string
  emoji?: { unicode?: string }
}

const themeClass = () => (document.documentElement.getAttribute('data-tg-theme') === 'night' ? 'dark' : 'light')

export function EmojiPanel({ onPick }: EmojiPanelProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const pickRef = useRef(onPick)
  pickRef.current = onPick
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let cancelled = false
    let picker: HTMLElement | null = null
    const observer = new MutationObserver(() => {
      picker?.classList.remove('light', 'dark')
      picker?.classList.add(themeClass())
    })
    const onClick = (event: Event) => {
      const detail = (event as CustomEvent<EmojiClickDetail>).detail
      const emoji = detail?.unicode || detail?.emoji?.unicode
      if (emoji) pickRef.current(emoji)
    }

    ensureSubtleDigest()
    Promise.all([import('emoji-picker-element'), import('emoji-picker-element/i18n/zh_CN')])
      .then(([module, zh]) => {
        if (cancelled || !hostRef.current) return
        const element = new module.Picker({ locale: 'zh', dataSource: '/emoji-data-zh.json', i18n: zh.default })
        element.classList.add('tg-compose__emoji-picker', themeClass())
        element.addEventListener('emoji-click', onClick)
        hostRef.current.append(element)
        picker = element
        observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-tg-theme'] })
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })

    return () => {
      cancelled = true
      observer.disconnect()
      picker?.removeEventListener('emoji-click', onClick)
      picker?.remove()
    }
  }, [])

  return (
    <div className="tg-compose__emoji" ref={hostRef}>
      {failed ? <p className="tg-compose__emoji-error">{t('w.composer.a8d4f7')}</p> : null}
    </div>
  )
}
