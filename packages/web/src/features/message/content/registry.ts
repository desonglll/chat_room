/**
 * Message content registry: maps a message to the component that draws its body.
 *
 * `createContentRegistry` is the pure mechanism (tests make their own); `messageContent` is
 * the app-wide instance the bubble reads, created with the five built-in kinds in
 * `builtins.ts`. A later task plugs in with
 *
 *     registerMessageContent('voice', VoiceContent, { match: (m) => isVoice(m), priority: 30 })
 *
 * and gets the whole bubble (header, quote, reactions, meta, menu) for free.
 */
import type { ComponentType } from 'react'
import type { BroadcastMessage } from '@tg/core'
import type { ContentKind, MessageContentProps, RegisterOptions } from './contentTypes'

export interface ContentRegistry {
  /** Adds or replaces `kind`. Returns an undo, for tests and hot reload. */
  register(kind: string, component: ComponentType<MessageContentProps>, options?: RegisterOptions): () => void
  /** The winning kind for `message`, or `undefined` when nothing matches. */
  resolve(message: BroadcastMessage): ContentKind | undefined
  kinds(): readonly string[]
}

function discriminator(message: BroadcastMessage): unknown {
  return (message as unknown as { kind?: unknown }).kind
}

function asFunction<T>(value: T | ((message: BroadcastMessage) => T) | undefined, fallback: T) {
  if (typeof value === 'function') return value as (message: BroadcastMessage) => T
  const fixed = value ?? fallback
  return () => fixed
}

export function createContentRegistry(): ContentRegistry {
  // Insertion order is the tie-break, so a Map keyed by kind, re-inserted on replace.
  const entries = new Map<string, ContentKind>()

  return {
    register(kind, component, options = {}) {
      const entry: ContentKind = {
        kind,
        component,
        match: options.match ?? ((message) => discriminator(message) === kind),
        frame: asFunction(options.frame, 'bubble'),
        metaPlacement: asFunction(options.metaPlacement, 'inline'),
        leadingMedia: options.leadingMedia ?? false,
        priority: options.priority ?? 0,
      }
      const previous = entries.get(kind)
      entries.delete(kind)
      entries.set(kind, entry)
      return () => {
        if (entries.get(kind) !== entry) return
        entries.delete(kind)
        if (previous !== undefined) entries.set(kind, previous)
      }
    },
    resolve(message) {
      let winner: ContentKind | undefined
      for (const entry of entries.values()) {
        if (!entry.match(message)) continue
        if (winner === undefined || entry.priority >= winner.priority) winner = entry
      }
      return winner
    },
    kinds() {
      return [...entries.keys()]
    },
  }
}
