/**
 * Sticker search over the local library, as Telegram's panel does it:
 *  - an emoji query → every installed sticker answering to it (`suggestStickers`);
 *  - a text query → installed sets whose title or short name contains it
 *    (case-insensitive), shown as whole sections;
 *  - a query shaped like a set short name that no installed set has → `remoteShortName`,
 *    which the tab resolves with `GET /api/sticker-sets/:short_name` to offer an install.
 */
import { suggestStickers, type StickerSet, type StickerState } from '@tg/core'
import { singleEmoji } from '../suggest/singleEmoji'
import type { PanelSection } from './panelLayout'
import { t } from '../../../i18n/index'

export interface StickerSearchResult {
  sections: PanelSection[]
  /** A short name worth looking up on the server, or `null`. */
  remoteShortName: string | null
}

const SHORT_NAME = /^[a-z][a-z0-9_]{0,63}$/

export function searchStickers(
  query: string,
  library: Pick<StickerState, 'sets' | 'recent' | 'favorites'>,
  activeSets: readonly StickerSet[],
): StickerSearchResult {
  const trimmed = query.trim()
  if (!trimmed) return { sections: [], remoteShortName: null }
  const emoji = singleEmoji(trimmed)
  if (emoji !== null) {
    const stickers = suggestStickers(library, emoji, 200)
    return { sections: [{ id: 'search', title: t('w.sticker.7ab919', emoji), stickers }], remoteShortName: null }
  }
  const needle = trimmed.toLocaleLowerCase()
  const sets = activeSets.filter(
    (set) => set.title.toLocaleLowerCase().includes(needle) || set.short_name.includes(needle),
  )
  const exact = library.sets.some((set) => set.short_name === needle)
  return {
    sections: sets.map((set) => ({ id: set.id, title: set.title, stickers: set.stickers })),
    remoteShortName: !exact && SHORT_NAME.test(needle) ? needle : null,
  }
}
