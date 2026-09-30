/**
 * A very small HTML-inspection helper for the ARIA contract tests.
 *
 * `bun test` has no DOM implementation, and adding one (happy-dom / jsdom) is a new dependency and
 * therefore not this task's call - see docs/devlog/TG-010.md § Blockers. What CAN be asserted
 * without one is the rendered markup, via `react-dom/server`, and that is where every ARIA
 * relationship lives: roles, names, `aria-*` state, and the id wiring between a label and its
 * control. The behavioural half (focus movement, key handling) is covered by the pure unit tests
 * over `rovingFocus` / `focusOrder` / `positioning`, which is why those modules are pure.
 */
export interface OpenTag {
  name: string
  attributes: Record<string, string>
}

const TAG = /<([a-zA-Z][\w-]*)((?:\s+[^\s=/>]+(?:="[^"]*")?)*)\s*\/?>/g
const ATTRIBUTE = /([^\s=/>]+)(?:="([^"]*)")?/g

/** Every opening tag in document order, with its attributes decoded enough for assertions. */
export function openTags(html: string): OpenTag[] {
  const tags: OpenTag[] = []
  for (const match of html.matchAll(TAG)) {
    const attributes: Record<string, string> = {}
    for (const attribute of (match[2] ?? '').matchAll(ATTRIBUTE)) {
      const name = attribute[1]
      if (name === undefined) continue
      attributes[name] = (attribute[2] ?? '').replace(/&quot;/g, '"').replace(/&amp;/g, '&')
    }
    tags.push({ name: match[1] as string, attributes })
  }
  return tags
}

/** Tags whose attributes all match the given subset. */
export function find(html: string, where: Partial<OpenTag> & { attributes?: Record<string, string> }): OpenTag[] {
  return openTags(html).filter((tag) => {
    if (where.name !== undefined && tag.name !== where.name) return false
    for (const [key, value] of Object.entries(where.attributes ?? {})) {
      if (tag.attributes[key] !== value) return false
    }
    return true
  })
}

/** The first tag with the given attribute, or undefined. */
export function withAttribute(html: string, attribute: string): OpenTag | undefined {
  return openTags(html).find((tag) => Object.hasOwn(tag.attributes, attribute))
}

/** All tags carrying the given `role`. */
export function byRole(html: string, role: string): OpenTag[] {
  return find(html, { attributes: { role } })
}

/** Text content of the element carrying `id`, so a label/description reference can be resolved. */
export function textOfId(html: string, id: string): string | null {
  const pattern = new RegExp(`<([a-zA-Z][\\w-]*)[^>]*\\bid="${id}"[^>]*>([\\s\\S]*?)</\\1>`, 'u')
  const match = pattern.exec(html)
  return match === null ? null : (match[2] ?? '').replace(/<[^>]*>/g, '').trim()
}
