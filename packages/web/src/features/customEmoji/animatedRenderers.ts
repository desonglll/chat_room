/**
 * The seam for animated custom emoji. TG-301 owns the TGS (Lottie) and WebM renderers; it
 * plugs them in here, and until it does an animated emoji shows its fallback character.
 *
 *     registerAnimatedEmojiRenderer('tgs', ({ src, size, fallback }) => <TgsPlayer … />)
 *
 * A renderer must stay inside `size`×`size`, loop, pause when off-screen and honour
 * `prefers-reduced-motion` (show the first frame). It must not draw text: the surrounding
 * `InlineCustomEmoji` already carries the fallback for copy and screen readers.
 */
import type { ComponentType } from 'react'
import type { CustomEmojiFormat } from '@tg/core'

export interface AnimatedEmojiRendererProps {
  src: string
  size: number
  fallback: string
}

export type AnimatedFormat = Exclude<CustomEmojiFormat, 'webp'>

const renderers = new Map<AnimatedFormat, ComponentType<AnimatedEmojiRendererProps>>()

/** Returns an undo, for tests and hot reload. */
export function registerAnimatedEmojiRenderer(
  format: AnimatedFormat,
  component: ComponentType<AnimatedEmojiRendererProps>,
): () => void {
  const previous = renderers.get(format)
  renderers.set(format, component)
  return () => {
    if (renderers.get(format) !== component) return
    if (previous === undefined) renderers.delete(format)
    else renderers.set(format, previous)
  }
}

export function animatedEmojiRenderer(format: AnimatedFormat): ComponentType<AnimatedEmojiRendererProps> | undefined {
  return renderers.get(format)
}
