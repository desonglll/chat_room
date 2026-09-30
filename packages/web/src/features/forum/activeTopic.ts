/**
 * The forum topic currently open per chat, for the HTTP send paths that sit outside the
 * chat session (attachment upload, sticker, poll). The topic view registers while mounted;
 * General and non-forum chats register nothing, so the send paths omit `topic_id`.
 */
const active = new Map<string, string>()

/** Returns the unregister function; a later registration for the same chat wins. */
export function setActiveTopic(chatId: string, topicId: string | null): () => void {
  if (topicId === null) {
    active.delete(chatId)
    return () => {}
  }
  active.set(chatId, topicId)
  return () => {
    if (active.get(chatId) === topicId) active.delete(chatId)
  }
}

/** The open non-General topic of a chat, or null (General / not a forum / not open). */
export function activeTopicId(chatId: string): string | null {
  return active.get(chatId) ?? null
}
