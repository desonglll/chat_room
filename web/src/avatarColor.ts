// Deterministic per-identity color, Telegram-style — the same username/room
// always maps to the same circle color, so people are visually distinguishable
// at a glance instead of every avatar being the same flat gray.
const PALETTE = ['#e17076', '#f5a35c', '#a695e7', '#7bc862', '#42b3ae', '#65aadd', '#ee7aae', '#8e85ee']

export function avatarColor(seed: string): string {
  let hash = 0
  for (let index = 0; index < seed.length; index += 1) {
    hash = (hash * 31 + seed.charCodeAt(index)) >>> 0
  }
  return PALETTE[hash % PALETTE.length]
}
