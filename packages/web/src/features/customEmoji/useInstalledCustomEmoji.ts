/**
 * The viewer's installed custom emoji sets for the pickers. Loading them also seeds the
 * emoji cache, so every glyph in the grid renders without a resolve request.
 */
import { useEffect, useState } from 'react'
import {
  authStore,
  listInstalledCustomEmoji,
  selectToken,
  type CustomEmoji,
  type CustomEmojiSet,
  type InstalledCustomEmojiSets,
} from '@tg/core'
import { apiClient } from '../../app/client'
import { customEmojiServices } from './services'

export type InstalledState = { status: 'loading' } | { status: 'failed' } | { status: 'ready'; sets: CustomEmojiSet[] }

export type InstalledLoader = () => Promise<InstalledCustomEmojiSets>

const defaultLoader: InstalledLoader = () => listInstalledCustomEmoji(apiClient, selectToken(authStore.getState()))

/** Flatten sets into cache entries keyed by emoji id. */
export function catalogueEntries(sets: readonly CustomEmojiSet[]): Array<[string, CustomEmoji]> {
  return sets.flatMap((set) =>
    set.stickers.map((sticker): [string, CustomEmoji] => [
      sticker.id,
      {
        id: sticker.id,
        set_id: set.id,
        set_short_name: set.short_name,
        emoji: sticker.emoji,
        format: sticker.format,
        width: sticker.width,
        height: sticker.height,
        file_url: sticker.file_url,
      },
    ]),
  )
}

export function useInstalledCustomEmoji(load: InstalledLoader = defaultLoader): InstalledState {
  const [state, setState] = useState<InstalledState>({ status: 'loading' })
  useEffect(() => {
    let cancelled = false
    load().then(
      (library) => {
        if (cancelled) return
        customEmojiServices().emoji.seed(catalogueEntries(library.sets))
        setState({ status: 'ready', sets: library.sets })
      },
      () => {
        if (!cancelled) setState({ status: 'failed' })
      },
    )
    return () => {
      cancelled = true
    }
  }, [load])
  return state
}
