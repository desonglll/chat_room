/**
 * TG-403 side-effect module, imported once by `main.tsx`: a collapsed album row renders as a
 * mosaic. Above the built-in image/video kinds (20) — an album representative is also an
 * image message — and below the recalled placeholder (100).
 */
import { registerMessageContent } from '../message'
import { AlbumContent, albumHasCaption } from './AlbumContent'
import { albumItemsOf } from './albumCollapse'
import './album.css'

registerMessageContent('album', AlbumContent, {
  match: (message) => albumItemsOf(message) !== undefined,
  frame: 'media',
  metaPlacement: (message) => (albumHasCaption(message) ? 'inline' : 'overlay'),
  leadingMedia: true,
  priority: 40,
})
