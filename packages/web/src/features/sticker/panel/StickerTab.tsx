/**
 * The 贴纸 tab of the media panel: search on top, the set rail on the left, the virtual
 * sticker grid (favorites, recents, then every installed set) on the right. A right click
 * on a sticker opens send / favorite / remove-from-recents / view-pack.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { selectActiveStickerSets, stickerStore, type Sticker, type StickerSet } from '@tg/core'
import { Button, Menu, Spinner, TextField, type MenuItem } from '@tg/ui'
import { useStore } from 'zustand/react'
import { SearchGlyph } from '../icons'
import { stickerLibrary } from '../stickerLibrary'
import { openStickerSet, openStickerSettings } from '../overlayStore'
import type { PanelSection } from './panelLayout'
import { SetRail, type RailEntry } from './SetRail'
import { StickerGrid, type StickerGridHandle } from './StickerGrid'
import { searchStickers } from './stickerSearch'
import { useRemoteSet } from './useRemoteSet'

export interface StickerTabProps {
  onSend(sticker: Sticker): void
  canSend: boolean
}

function librarySections(favorites: Sticker[], recent: Sticker[], sets: StickerSet[]): PanelSection[] {
  return [
    { id: 'favorites', title: '收藏', stickers: favorites },
    { id: 'recent', title: '最近使用', stickers: recent },
    ...sets.map((set) => ({ id: set.id, title: set.title, stickers: set.stickers })),
  ]
}

export function StickerTab({ onSend, canSend }: StickerTabProps) {
  const status = useStore(stickerStore, (state) => state.status)
  const all = useStore(stickerStore, (state) => state.sets)
  const recent = useStore(stickerStore, (state) => state.recent)
  const favorites = useStore(stickerStore, (state) => state.favorites)
  const sets = useMemo(() => selectActiveStickerSets({ sets: all }), [all])
  const [query, setQuery] = useState('')
  const [active, setActive] = useState<string | null>(null)
  const [menu, setMenu] = useState<{ sticker: Sticker; x: number; y: number } | null>(null)
  const gridRef = useRef<StickerGridHandle | null>(null)

  useEffect(() => {
    void stickerLibrary()
      .ensureLoaded()
      .catch(() => undefined)
  }, [])

  const search = useMemo(
    () => searchStickers(query, { sets: all, recent, favorites }, sets),
    [query, all, recent, favorites, sets],
  )
  const remote = useRemoteSet(search.remoteShortName)
  const sections = useMemo(
    () => (query.trim() ? search.sections : librarySections(favorites, recent, sets)),
    [query, search, favorites, recent, sets],
  )
  const rail = useMemo<RailEntry[]>(() => {
    const entries: RailEntry[] = []
    if (favorites.length > 0) entries.push({ id: 'favorites', title: '收藏', icon: 'favorites' })
    if (recent.length > 0) entries.push({ id: 'recent', title: '最近使用', icon: 'recent' })
    for (const set of sets) {
      const first = set.stickers[0]
      if (first) entries.push({ id: set.id, title: set.title, icon: first })
    }
    return entries
  }, [favorites, recent, sets])

  const menuItems = useMemo<MenuItem[]>(() => {
    if (!menu) return []
    const { sticker } = menu
    const favorite = favorites.some((entry) => entry.id === sticker.id)
    const inRecent = recent.some((entry) => entry.id === sticker.id)
    const owner = all.find((set) => set.id === sticker.set_id)
    const items: MenuItem[] = [
      { id: 'send', label: '发送贴纸', disabled: !canSend, onSelect: () => onSend(sticker) },
      {
        id: 'favorite',
        label: favorite ? '取消收藏' : '添加到收藏',
        onSelect: () =>
          void stickerLibrary()
            .toggleFavorite(sticker)
            .catch(() => undefined),
      },
    ]
    if (inRecent) {
      items.push({
        id: 'recent',
        label: '从最近使用中移除',
        onSelect: () =>
          void stickerLibrary()
            .removeRecent(sticker.id)
            .catch(() => undefined),
      })
    }
    if (owner) items.push({ id: 'set', label: '查看贴纸包', onSelect: () => openStickerSet(owner.short_name) })
    return items
  }, [menu, favorites, recent, all, canSend, onSend])

  const empty = sections.every((section) => section.stickers.length === 0)
  return (
    <div className="tg-sticker-tab">
      <div className="tg-sticker-tab__search">
        <TextField
          size="sm"
          fullWidth
          value={query}
          placeholder="搜索贴纸"
          aria-label="搜索贴纸"
          startAdornment={<SearchGlyph />}
          clearable
          clearLabel="清除搜索"
          onClear={() => setQuery('')}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="tg-sticker-tab__body">
        {query.trim() ? null : (
          <SetRail
            entries={rail}
            active={active}
            onSelect={(id) => gridRef.current?.scrollToSection(id)}
            onSettings={openStickerSettings}
          />
        )}
        <div className="tg-sticker-tab__main">
          {remote ? (
            <div className="tg-sticker-tab__remote">
              <span>{remote.title}</span>
              <Button size="sm" variant="tonal" onClick={() => openStickerSet(remote.short_name)}>
                查看
              </Button>
            </div>
          ) : null}
          {status === 'loading' && empty ? (
            <div className="tg-sticker-tab__state">
              <Spinner size="md" label="正在加载贴纸" />
            </div>
          ) : empty ? (
            <p className="tg-sticker-tab__state">
              {query.trim() ? '没有找到贴纸' : status === 'error' ? '贴纸加载失败' : '还没有贴纸包'}
            </p>
          ) : (
            <StickerGrid
              sections={sections}
              onPick={onSend}
              onMenu={(sticker, point) => setMenu({ sticker, ...point })}
              onActiveSection={setActive}
              handleRef={gridRef}
              disabled={!canSend}
              label={query.trim() ? '搜索结果' : '贴纸'}
            />
          )}
        </div>
      </div>
      <Menu
        open={menu !== null}
        onClose={() => setMenu(null)}
        anchor={menu ? { x: menu.x, y: menu.y } : null}
        items={menuItems}
        aria-label="贴纸操作"
      />
    </div>
  )
}
