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
      play: '播放',
      pause: '暂停',
      played: '已播放',
      buffered: '已缓冲',
      currentTime: '当前时间',
      duration: '时长',
      volume: '音量',
      mute: '静音',
      unmute: '取消静音',
      enterFullscreen: '全屏',
      exitFullscreen: '退出全屏',
      settings: '设置',
      menuBack: '返回',
      speed: '速度',
      normal: '正常',
      seek: '跳转',
    },
  })
  return { destroy: () => player.destroy() }
}
