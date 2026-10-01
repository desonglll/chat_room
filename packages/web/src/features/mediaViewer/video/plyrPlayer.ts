/**
 * plyr, loaded only when a video is first shown in the viewer (its JS and CSS are a separate
 * chunk). Two deliberate deviations from plyr's defaults, both forced by the server's CSP
 * (`src/security/headers.rs`: `connect-src 'self'`):
 *
 *  - the icon sprite is inlined from the installed package instead of fetched from
 *    cdn.plyr.io (plyr's `loadSprite` would XHR it and be blocked, leaving blank buttons);
 *    plyr then references the symbols in-page because its default `iconUrl` is cross-origin;
 *  - nothing else is fetched from a CDN (no `blankVideo` source swap: a player is created
 *    per medium and destroyed with it).
 *
 * The package does not export `plyr.svg`, hence the path into its directory.
 */
import Plyr from 'plyr'
import 'plyr/dist/plyr.css'
import sprite from '../../../../node_modules/plyr/dist/plyr.svg?raw'
import { t } from '../../../i18n/index'

const SPRITE_ID = 'sprite-plyr'

function ensureSprite(): void {
  if (document.getElementById(SPRITE_ID)) return
  const holder = document.createElement('div')
  holder.id = SPRITE_ID
  holder.hidden = true
  holder.innerHTML = sprite.replace(/<\?xml[^>]*>|<!DOCTYPE[^>]*>/gi, '')
  document.body.prepend(holder)
}

export interface VideoPlayerHandle {
  destroy(): void
}

export function createPlayer(video: HTMLVideoElement): VideoPlayerHandle {
  ensureSprite()
  const player = new Plyr(video, {
    controls: [
      'play-large',
      'play',
      'progress',
      'current-time',
      'duration',
      'mute',
      'volume',
      'settings',
      'fullscreen',
    ],
    settings: ['speed'],
    autoplay: true,
    loadSprite: false,
    clickToPlay: true,
    hideControls: true,
    resetOnEnd: false,
    keyboard: { focused: true, global: false },
    storage: { enabled: true, key: 'tg.player' },
    i18n: {
      play: t('w.mediaViewer.219253'),
      pause: t('w.mediaViewer.130448'),
      played: t('w.mediaViewer.2ce155'),
      buffered: t('w.mediaViewer.90a93e'),
      currentTime: t('w.mediaViewer.626e5b'),
      duration: t('w.mediaViewer.29d055'),
      volume: t('w.mediaViewer.1aa999'),
      mute: t('w.mediaViewer.afdbd1'),
      unmute: t('w.mediaViewer.59bc75'),
      enterFullscreen: t('w.mediaViewer.93c44f'),
      exitFullscreen: t('w.mediaViewer.a170c5'),
      settings: t('w.mediaViewer.7debf9'),
      menuBack: t('w.mediaViewer.11d024'),
      speed: t('w.mediaViewer.f2fdff'),
      normal: t('w.mediaViewer.f78d03'),
      seek: t('w.mediaViewer.28cd2e'),
    },
  })
  return { destroy: () => player.destroy() }
}
