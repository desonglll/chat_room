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
import { t } from '../../../i18n/index'

export interface StickerTabProps {
  onSend(sticker: Sticker): void
  canSend: boolean
}

function librarySections(favorites: Sticker[], recent: Sticker[], sets: StickerSet[]): PanelSection[] {
  return [
    { id: 'favorites', title: t('w.sticker.d07cee'), stickers: favorites },
    { id: 'recent', title: t('w.sticker.71265f'), stickers: recent },
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
    if (favorites.length > 0) entries.push({ id: 'favorites', title: t('w.sticker.d07cee'), icon: 'favorites' })
    if (recent.length > 0) entries.push({ id: 'recent', title: t('w.sticker.71265f'), icon: 'recent' })
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
      { id: 'send', label: t('w.sticker.8e5cc0'), disabled: !canSend, onSelect: () => onSend(sticker) },
      {
        id: 'favorite',
        label: favorite ? t('w.sticker.26acae') : t('w.sticker.143fe6'),
        onSelect: () =>
          void stickerLibrary()
            .toggleFavorite(sticker)
            .catch(() => undefined),
      },
    ]
    if (inRecent) {
      items.push({
        id: 'recent',
        label: t('w.sticker.bab462'),
        onSelect: () =>
          void stickerLibrary()
            .removeRecent(sticker.id)
            .catch(() => undefined),
      })
    }
    if (owner) items.push({ id: 'set', label: t('w.sticker.3abdbf'), onSelect: () => openStickerSet(owner.short_name) })
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
          placeholder={t('w.sticker.73a0ac')}
          aria-label={t('w.sticker.73a0ac')}
          startAdornment={<SearchGlyph />}
          clearable
          clearLabel={t('w.sticker.318ea1')}
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
                {t('w.sticker.f7acef')}
              </Button>
            </div>
          ) : null}
          {status === 'loading' && empty ? (
            <div className="tg-sticker-tab__state">
              <Spinner size="md" label={t('w.sticker.97ad37')} />
            </div>
          ) : empty ? (
            <p className="tg-sticker-tab__state">
              {query.trim()
                ? t('w.sticker.826fad')
                : status === 'error'
                  ? t('w.sticker.f9fd05')
                  : t('w.sticker.42c27c')}
            </p>
          ) : (
            <StickerGrid
              sections={sections}
              onPick={onSend}
              onMenu={(sticker, point) => setMenu({ sticker, ...point })}
              onActiveSection={setActive}
              handleRef={gridRef}
              disabled={!canSend}
              label={query.trim() ? t('w.sticker.2f17ec') : t('w.sticker.f7c0f3')}
            />
          )}
        </div>
      </div>
      <Menu
        open={menu !== null}
        onClose={() => setMenu(null)}
        anchor={menu ? { x: menu.x, y: menu.y } : null}
        items={menuItems}
        aria-label={t('w.sticker.7dfa8e')}
      />
    </div>
  )
}
