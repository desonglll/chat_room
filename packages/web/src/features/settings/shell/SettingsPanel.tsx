/**
 * The settings panel (TG-110): Telegram's settings column. It slides in over the left column
 * (desktop; wider than a collapsed column) or covers the screen (mobile, where the column is
 * the screen), and slides back out on close. Each pushed view slides in from the right; back
 * slides the previous one in from the left. Reduced motion swaps every slide for a fade.
 *
 * Esc closes the panel when focus is inside it (or nowhere), unless an inner overlay — a
 * `Menu`, a `Modal` — consumed that Escape first (`@tg/ui` marks it `defaultPrevented`).
 * Focus moves to the view's title on every navigation and back to the opener on close.
 */
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useStore } from 'zustand/react'
import './builtinPages'
import './settings.css'
import './settingsPages.css'
import { settingsNavigation, type SettingsView } from './settingsNavigation'
import { findSettingsPage, settingsPagesSnapshot, subscribeSettingsPages } from './settingsRegistry'
import { PageView, RootView, SectionView, sectionById, type ViewContext } from './SettingsViews'

/** Safety net for a missed `animationend` (tab hidden mid-animation). */
const EXIT_FALLBACK_MS = 700

const viewKey = (depth: number, view: SettingsView | undefined) => (view ? `${depth}:${view.kind}:${view.id}` : 'root')

export function SettingsPanel() {
  const open = useStore(settingsNavigation, (state) => state.open)
  const stack = useStore(settingsNavigation, (state) => state.stack)
  const pages = useSyncExternalStore(subscribeSettingsPages, settingsPagesSnapshot, settingsPagesSnapshot)
  const [mounted, setMounted] = useState(open)
  if (open && !mounted) setMounted(true)
  const panelRef = useRef<HTMLElement | null>(null)
  const openerRef = useRef<HTMLElement | null>(null)
  const top = stack[stack.length - 1]
  const key = viewKey(stack.length, top)
  // The slide direction is decided once, when the view changes, and kept for that view's life:
  // recomputing it on a later render would swap the running animation out from under it.
  const lastView = useRef({ key, depth: stack.length, direction: 'none' as 'none' | 'forward' | 'back' })
  if (lastView.current.key !== key) {
    const direction = stack.length >= lastView.current.depth ? 'forward' : 'back'
    lastView.current = { key, depth: stack.length, direction }
  }
  const direction = lastView.current.direction

  // Remember the opener, and hand focus back to it once the panel closes.
  useLayoutEffect(() => {
    if (open) {
      const active = document.activeElement
      if (active instanceof HTMLElement && !panelRef.current?.contains(active)) openerRef.current = active
      return
    }
    if (panelRef.current?.contains(document.activeElement)) openerRef.current?.focus()
  }, [open])

  useEffect(() => {
    if (!open) return
    const panel = panelRef.current
    const title = panel?.querySelector<HTMLElement>('.tg-settings__view .tg-settings__title')
    ;(title ?? panel?.querySelector<HTMLElement>('.tg-settings__view'))?.focus({ preventScroll: true })
  }, [open, key])

  useEffect(() => {
    if (!open) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return
      const active = document.activeElement
      const inside = active === null || active === document.body || panelRef.current?.contains(active)
      if (!inside) return
      event.preventDefault()
      settingsNavigation.getState().close()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [open])

  useEffect(() => {
    if (open || !mounted) return
    const timer = setTimeout(() => setMounted(false), EXIT_FALLBACK_MS)
    return () => clearTimeout(timer)
  }, [open, mounted])

  if (!mounted) return null

  const navigation = settingsNavigation.getState()
  const context: ViewContext = { pages, push: navigation.push, back: navigation.back, close: navigation.close }
  let view = <RootView context={context} />
  if (top?.kind === 'section') view = <SectionView section={sectionById(top.id)} context={context} />
  if (top?.kind === 'page') {
    const page = findSettingsPage(pages, top.id)
    if (page) view = <PageView page={page} context={context} />
  }

  return (
    <section
      ref={panelRef}
      className="tg-settings"
      aria-label="设置"
      data-closing={open ? undefined : ''}
      inert={!open}
      onAnimationEnd={(event) => {
        if (event.target === event.currentTarget && !open) setMounted(false)
      }}
    >
      <div key={key} className="tg-settings__view" data-direction={direction} tabIndex={-1}>
        {view}
      </div>
    </section>
  )
}

export default SettingsPanel
