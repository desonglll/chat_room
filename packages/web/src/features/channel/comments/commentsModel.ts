import { t } from '../../../i18n/index'
/** TG-203 pure helpers for the comments entry and thread. */

/** The entry's label: Telegram says «发表评论» on an empty thread and counts otherwise. */
export function commentsLabel(count: number): string {
  return count > 0 ? t('w.channel.764250', count) : t('w.channel.78d9c8')
}

/** Whose comment a reply in the thread answers, for the «回复 X» line; '' when it answers the post. */
export function replyTarget(
  replyToId: string | undefined,
  rootId: string,
  byId: ReadonlyMap<string, { sender: string }>,
): string {
  if (!replyToId || replyToId === rootId) return ''
  return byId.get(replyToId)?.sender ?? ''
}
