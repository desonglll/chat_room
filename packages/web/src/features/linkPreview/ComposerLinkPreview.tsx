/**
 * TG-408: while the draft contains a link, its card shows above the input (debounced); ✕ sends
 * the message without a card. Nothing is fetched by the browser itself: the server fetches.
 */
import { useEffect, useState } from 'react'
import type { LinkPreview } from '@tg/core'
import { firstLink } from '@tg/core'
import { useStore } from 'zustand/react'
import { linkPreviewApi } from './linkPreviewApi'
import { LinkPreviewCard } from './LinkPreviewCard'
import { dismissComposerLink, linkPreviewStore } from './linkPreviewStore'

const DEBOUNCE_MS = 500

export function ComposerLinkPreview({ chatId, text }: { chatId: string; text: string }) {
  const link = firstLink(text)
  const dismissed = useStore(linkPreviewStore, (state) => state.dismissed[chatId])
  const [preview, setPreview] = useState<{ link: string; card: LinkPreview } | null>(null)

  useEffect(() => {
    if (!link || link === dismissed) return
    let cancelled = false
    const timer = setTimeout(() => {
      linkPreviewApi.get(link).then(
        (card) => {
          if (!cancelled) setPreview(card ? { link, card } : null)
        },
        () => undefined,
      )
    }, DEBOUNCE_MS)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [link, dismissed])

  if (!link || link === dismissed || preview?.link !== link) return null
  return (
    <div className="tg-compose-link">
      <LinkPreviewCard preview={preview.card} onDismiss={() => dismissComposerLink(chatId, link)} />
    </div>
  )
}
