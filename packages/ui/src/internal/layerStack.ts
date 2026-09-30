/**
 * A module-level stack of open overlays, so that Escape closes exactly the topmost one.
 *
 * Without this, a `Menu` opened inside a `Modal` would close both on a single Escape, because
 * each would have its own document-level key listener. The stack is deliberately not React
 * state: ordering must be observable synchronously from an event handler, before any render.
 */
export type LayerId = { readonly tag: 'layer' }

const stack: LayerId[] = []

export function pushLayer(): LayerId {
  const id: LayerId = { tag: 'layer' }
  stack.push(id)
  return id
}

export function removeLayer(id: LayerId): void {
  const at = stack.indexOf(id)
  if (at !== -1) stack.splice(at, 1)
}

export function isTopLayer(id: LayerId): boolean {
  return stack.length > 0 && stack[stack.length - 1] === id
}

export function layerDepth(): number {
  return stack.length
}

/** Test seam: the stack is module state and a leaked entry would be invisible otherwise. */
export function resetLayers(): void {
  stack.length = 0
}
