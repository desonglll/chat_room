/** TG-203 pure helpers for the comments entry and thread. */

/** The entry's label: Telegram says «发表评论» on an empty thread and counts otherwise. */
export function commentsLabel(count: number): string {
  return count > 0 ? `${count} 条评论` : '发表评论'
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
