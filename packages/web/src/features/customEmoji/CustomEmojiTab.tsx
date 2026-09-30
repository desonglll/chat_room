/**
 * The "自定义" tab of the composer's emoji panel: the viewer's installed custom emoji
 * sets. The host inserts the pick with `useEntityDraft().insert` (see devlog TG-304,
 * "Integration patch list").
 */
import { CustomEmojiGrid, type PickedCustomEmoji } from './CustomEmojiGrid'
import { useInstalledCustomEmoji, type InstalledLoader } from './useInstalledCustomEmoji'

export interface CustomEmojiTabProps {
  onPick(emoji: PickedCustomEmoji): void
  /** Injected in tests; defaults to `GET /api/custom-emoji/installed`. */
  load?: InstalledLoader | undefined
}

export function CustomEmojiTab({ onPick, load }: CustomEmojiTabProps) {
  const state = useInstalledCustomEmoji(load)
  if (state.status === 'loading') return <p className="tg-custom-emoji-grid__empty">加载中…</p>
  if (state.status === 'failed') return <p className="tg-custom-emoji-grid__empty">自定义表情加载失败</p>
  return <CustomEmojiGrid sets={state.sets} onPick={onPick} />
}
