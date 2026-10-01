/**
 * Shared content: a sticky tab strip over one panel per tab (members for groups, then
 * media / files / links / voice / GIF).
 *
 * Independence, as the acceptance asks:
 * - pagination — each tab reads its own pager (own cursor, own `loadMore`), and only the
 *   ACTIVE tab's sentinel can trigger a load;
 * - scrolling — a visited panel stays mounted (hidden), and the panel body's scroll offset
 *   is remembered per tab. Switching tabs while the strip is stuck restores that tab's
 *   own offset (never above the strip); switching above it leaves the profile where it is.
 *   That is Telegram's behaviour for its single scrolling info column.
 */
import type { ReactNode, RefObject } from 'react'
import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { ChatMembership } from '@tg/core'
import { Spinner, Tabs } from '@tg/ui'
import { useStore } from 'zustand/react'
import type { SharedPager } from './sharedPager'
import { MemberRemove } from '../chatLifecycle'
import { FileRow, GifTile, LinkRow, MediaTile, MemberRow, VoiceRow } from './sharedItems'
import type { SharedFile, SharedLink, SharedTabId } from './sharedSources'
import { SHARED_TABS } from './sharedSources'
import type { ChatInfoPagers } from './useChatInfo'
import { t } from '../../i18n/index'

export type InfoTabId = SharedTabId | 'members'

const MEMBERS_TAB = {
  id: 'members' as const,
  get label() {
    return t('w.chatInfo.c1ee9f')
  },
  get empty() {
    return t('w.chatInfo.4fc505')
  },
}
/** Start the next page this far before the sentinel scrolls into view. */
const PREFETCH_MARGIN_PX = 480

export function infoTabs(showMembers: boolean): ReadonlyArray<{ id: InfoTabId; label: string; empty: string }> {
  return showMembers ? [MEMBERS_TAB, ...SHARED_TABS] : SHARED_TABS
}

interface TabPanelProps<T> {
  id: InfoTabId
  label: string
  empty: string
  active: boolean
  layout: 'grid' | 'list'
  pager: SharedPager<T>
  scrollerRef: RefObject<HTMLElement | null>
  render(item: T): ReactNode
  keyOf(item: T): string
}

function SharedTabPanel<T>({ id, label, empty, active, layout, pager, scrollerRef, render, keyOf }: TabPanelProps<T>) {
  const { items, started, loading, done, error } = useStore(pager)
  const sentinel = useRef<HTMLDivElement>(null)

  // First page on first activation only: an unvisited tab costs no request.
  useEffect(() => {
    if (active && !started) void pager.getState().loadMore()
  }, [active, started, pager])

  // Re-armed after every page, so a sentinel still in view after a short page loads again.
  useEffect(() => {
    const target = sentinel.current
    if (!active || done || error || !target || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) void pager.getState().loadMore()
      },
      { root: scrollerRef.current, rootMargin: `0px 0px ${PREFETCH_MARGIN_PX}px 0px` },
    )
    observer.observe(target)
    return () => observer.disconnect()
  }, [active, done, error, items.length, pager, scrollerRef])

  const List = layout === 'grid' ? 'div' : 'ul'
  return (
    <section
      className="tg-chatinfo__panel"
      role="tabpanel"
      aria-label={label}
      data-tab={id}
      data-layout={layout}
      hidden={!active}
    >
      {items.length > 0 ? (
        <List className={layout === 'grid' ? 'tg-chatinfo__grid' : 'tg-chatinfo__list'}>
          {items.map((item) => (
            <Fragment key={keyOf(item)}>{render(item)}</Fragment>
          ))}
        </List>
      ) : null}
      {started && done && items.length === 0 && !error ? <p className="tg-chatinfo__empty">{empty}</p> : null}
      {error ? (
        <button type="button" className="tg-chatinfo__retry" onClick={() => void pager.getState().loadMore()}>
          {t('w.chatInfo.560c03')}
        </button>
      ) : null}
      {loading ? (
        <div className="tg-chatinfo__loading">
          <Spinner size="sm" label={t('w.chatInfo.ce56f6')} />
        </div>
      ) : null}
      <div ref={sentinel} className="tg-chatinfo__sentinel" aria-hidden="true" />
    </section>
  )
}

export interface SharedSectionProps {
  chatId: string
  pagers: ChatInfoPagers
  showMembers: boolean
  scrollerRef: RefObject<HTMLElement | null>
}

export function SharedSection({ chatId, pagers, showMembers, scrollerRef }: SharedSectionProps) {
  const tabs = useMemo(() => infoTabs(showMembers), [showMembers])
  const [active, setActive] = useState<InfoTabId>(tabs[0]?.id ?? 'media')
  const [visited, setVisited] = useState<ReadonlySet<InfoTabId>>(() => new Set([active]))
  const offsets = useRef(new Map<InfoTabId, number>())
  const stickPoint = useRef<HTMLDivElement>(null)

  const select = (id: string) => {
    const next = id as InfoTabId
    if (next === active) return
    const scroller = scrollerRef.current
    if (scroller) offsets.current.set(active, scroller.scrollTop)
    setVisited((current) => (current.has(next) ? current : new Set([...current, next])))
    setActive(next)
  }

  useLayoutEffect(() => {
    const scroller = scrollerRef.current
    const marker = stickPoint.current
    if (!scroller || !marker) return
    const stick = marker.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop
    if (scroller.scrollTop >= stick - 1) scroller.scrollTop = Math.max(stick, offsets.current.get(active) ?? stick)
  }, [active, scrollerRef])

  const fileKey = (file: SharedFile) => file.key
  const panel = (id: InfoTabId, label: string, empty: string): ReactNode => {
    if (!visited.has(id)) return null
    const common = { id, label, empty, active: id === active, scrollerRef }
    switch (id) {
      case 'members':
        return (
          <SharedTabPanel<ChatMembership>
            key={id}
            {...common}
            layout="list"
            pager={pagers.members}
            keyOf={(member) => member.user_id}
            render={(member) => <MemberRow member={member} action={<MemberRemove chatId={chatId} member={member} />} />}
          />
        )
      case 'media':
        return (
          <SharedTabPanel<SharedFile>
            key={id}
            {...common}
            layout="grid"
            pager={pagers.media}
            keyOf={fileKey}
            render={(file) => <MediaTile chatId={chatId} file={file} />}
          />
        )
      case 'gif':
        return (
          <SharedTabPanel<SharedFile>
            key={id}
            {...common}
            layout="grid"
            pager={pagers.gif}
            keyOf={fileKey}
            render={(file) => <GifTile file={file} />}
          />
        )
      case 'files':
        return (
          <SharedTabPanel<SharedFile>
            key={id}
            {...common}
            layout="list"
            pager={pagers.files}
            keyOf={fileKey}
            render={(file) => <FileRow file={file} />}
          />
        )
      case 'music':
        return (
          <SharedTabPanel<SharedFile>
            key={id}
            {...common}
            layout="list"
            pager={pagers.music}
            keyOf={fileKey}
            render={(file) => <FileRow file={file} />}
          />
        )
      case 'voice':
        return (
          <SharedTabPanel<SharedFile>
            key={id}
            {...common}
            layout="list"
            pager={pagers.voice}
            keyOf={fileKey}
            render={(file) => <VoiceRow file={file} />}
          />
        )
      case 'links':
        return (
          <SharedTabPanel<SharedLink>
            key={id}
            {...common}
            layout="list"
            pager={pagers.links}
            keyOf={(link) => link.key}
            render={(link) => <LinkRow link={link} />}
          />
        )
    }
  }

  return (
    <div className="tg-chatinfo__shared">
      <div ref={stickPoint} aria-hidden="true" />
      <Tabs
        className="tg-chatinfo__tabs"
        items={tabs}
        value={active}
        onValueChange={select}
        stretch
        aria-label={t('w.chatInfo.8ac831')}
      />
      {tabs.map((tab) => panel(tab.id, tab.label, tab.empty))}
    </div>
  )
}
