/**
 * The app's voice player instance: a real `Audio` element, animation frames, the open
 * chats' timelines for "play next", and listened reporting when someone else's voice
 * message starts. The playback speed survives reloads (Telegram keeps it too).
 */
import { authStore, messageStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { browserStorage } from '../../app/platform'
import { createVoicePlayer, VOICE_RATES, type VoicePlayer, type VoiceRate } from './voicePlayer'
import { locateVoice, nextVoiceTrack } from './voiceTimeline'
import { reportListened } from './voiceStore'

const RATE_KEY = 'tg.voice.rate.v1'

function storedRate(): VoiceRate {
  const value = Number(browserStorage.getItem(RATE_KEY))
  return (VOICE_RATES as readonly number[]).includes(value) ? (value as VoiceRate) : 1
}

let instance: VoicePlayer | null = null

export function appVoicePlayer(): VoicePlayer {
  instance ??= createVoicePlayer({
    createAudio: () => {
      const audio = new Audio()
      audio.preload = 'auto'
      // Telegram keeps the voice's pitch at 1.5×/2×.
      audio.preservesPitch = true
      return audio
    },
    requestFrame: (callback) => window.requestAnimationFrame(callback),
    cancelFrame: (handle) => window.cancelAnimationFrame(handle as number),
    findNext: (messageId) => nextVoiceTrack(messageStore, messageId),
    onStart: (track) => {
      const located = locateVoice(messageStore, track.messageId)
      const session = authStore.getState().session
      if (!located || !session || located.message.sender_id === session.user.id) return
      if (located.message.voice?.listened) return
      reportListened(track.messageId, { client: apiClient, token: selectToken(authStore.getState()) })
    },
    initialRate: storedRate(),
    onRateChange: (rate) => browserStorage.setItem(RATE_KEY, String(rate)),
  })
  return instance
}
