/**
 * TG-1201: which reactions the viewer has just added, so the chip that a FIRST reaction mounts
 * can burst like Telegram's. A chip that was already on screen bursts on its own (chosen goes
 * false → true); a freshly mounted chip cannot tell the viewer's tap from history, so the tap
 * leaves a short-lived note here. Reading never consumes the note: React may run a state
 * initializer twice, and a remount inside the window bursting again is harmless.
 */

const WINDOW_MS = 3000
const notes = new Map<string, number>()

const keyOf = (messageId: string, emoji: string) => `${messageId}\u0000${emoji}`

export function noteOwnReaction(messageId: string, emoji: string, now = Date.now()): void {
  for (const [key, at] of notes) if (now - at > WINDOW_MS) notes.delete(key)
  notes.set(keyOf(messageId, emoji), now)
}

export function isFreshOwnReaction(messageId: string, emoji: string, now = Date.now()): boolean {
  const at = notes.get(keyOf(messageId, emoji))
  return at !== undefined && now - at <= WINDOW_MS
}
