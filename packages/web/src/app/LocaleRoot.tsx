/**
 * TG-510: re-renders the whole tree when the language changes. Keyed by locale so memoised
 * components re-read their copy too; the router, stores and session live outside React and
 * survive the switch (no page reload).
 */
import type { ReactNode } from 'react'
import { localeStore } from '@tg/core'
import { useStore } from 'zustand/react'

export function LocaleRoot({ children }: { children: ReactNode }) {
  const locale = useStore(localeStore, (state) => state.locale)
  return (
    <div key={locale} style={{ display: 'contents' }}>
      {children}
    </div>
  )
}
