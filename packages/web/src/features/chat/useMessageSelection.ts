/** Selection mode of one open chat: the first selected message enters it, none leaves it. */
import { useCallback, useEffect, useState } from 'react'

const EMPTY: ReadonlySet<string> = new Set()

export interface MessageSelection {
  selected: ReadonlySet<string>
  toggle(messageId: string): void
  clear(): void
}

export function useMessageSelection(chatId: string): MessageSelection {
  const [selected, setSelected] = useState<ReadonlySet<string>>(EMPTY)
  useEffect(() => setSelected(EMPTY), [chatId])
  const toggle = useCallback((messageId: string) => {
    setSelected((current) => {
      const next = new Set(current)
      if (!next.delete(messageId)) next.add(messageId)
      return next
    })
  }, [])
  const clear = useCallback(() => setSelected(EMPTY), [])
  return { selected, toggle, clear }
}
