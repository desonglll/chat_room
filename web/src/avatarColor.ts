// Muted colors keep identities distinct without turning the workspace into a
// field of competing accents.
const PALETTE = ['#47796e', '#607d9a', '#8b6f7f', '#927d56', '#6e8261', '#746f91']

export function avatarColor(seed: string): string {
  let hash = 0
  for (let index = 0; index < seed.length; index += 1) {
    hash = (hash * 31 + seed.charCodeAt(index)) >>> 0
  }
  return PALETTE[hash % PALETTE.length]
}
