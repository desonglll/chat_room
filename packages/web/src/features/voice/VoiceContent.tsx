/**
 * The voice message bubble body (TG-401), registered with TG-103's content registry by
 * `register.ts`: play/pause, the waveform with progress and drag seek, the duration (or the
 * position while playing), the unlistened dot, and the speed chip while this track is loaded.
 */
import type { BroadcastMessage, VoiceNote } from '@tg/core'
import { useStore } from 'zustand/react'
import type { MessageContentProps } from '../message'
import { appVoicePlayer } from './appVoicePlayer'
import type { VoicePlayer, VoiceRate, VoiceTrack } from './voicePlayer'
import { voiceStore } from './voiceStore'
import { VoiceWaveform } from './VoiceWaveform'
import { formatVoiceDuration, waveformBarCount } from './waveform'
import { PauseGlyph, PlayGlyph } from './glyphs'

export function VoiceContent({ message, metaSpacer }: MessageContentProps) {
  const voice = message.voice
  if (!voice || !message.attachment) return null
  return <VoiceMessage message={message} voice={voice} url={message.attachment.download_url} metaSpacer={metaSpacer} />
}

interface VoiceMessageProps {
  message: BroadcastMessage
  voice: VoiceNote
  url: string
  metaSpacer: MessageContentProps['metaSpacer']
}

function VoiceMessage({ message, voice, url, metaSpacer }: VoiceMessageProps) {
  const player = appVoicePlayer()
  const listenedLive = useStore(voiceStore, (state) => state.listened[message.message_id] === true)
  const loaded = useStore(player.store, (state) => state.current?.messageId === message.message_id)
  const playing = useStore(player.store, (state) => loaded && state.playing)
  const positionMs = useStore(player.store, (state) => (loaded ? state.positionMs : 0))
  const rate = useStore(player.store, (state) => state.rate)
  return (
    <VoiceBody
      track={{ messageId: message.message_id, url, durationMs: voice.duration_ms }}
      waveform={voice.waveform}
      listened={voice.listened || listenedLive}
      loaded={loaded}
      playing={playing}
      positionMs={positionMs}
      rate={rate}
      player={player}
      metaSpacer={metaSpacer}
    />
  )
}

export interface VoiceBodyProps {
  track: VoiceTrack
  waveform: readonly number[]
  listened: boolean
  loaded: boolean
  playing: boolean
  positionMs: number
  rate: VoiceRate
  player: Pick<VoicePlayer, 'toggle' | 'seek' | 'cycleRate'>
  metaSpacer: MessageContentProps['metaSpacer']
}

/** Stateless face of the bubble; `VoiceContent` feeds it from the player and the store. */
export function VoiceBody(props: VoiceBodyProps) {
  const { track, waveform, listened, loaded, playing, positionMs, rate, player, metaSpacer } = props
  const progress = track.durationMs > 0 ? Math.min(1, positionMs / track.durationMs) : 0
  const shown = loaded && positionMs > 0 ? positionMs : track.durationMs
  return (
    <div className="tg-voice" data-playing={playing || undefined}>
      <button
        type="button"
        className="tg-voice__play"
        aria-label={playing ? '暂停语音' : '播放语音'}
        onClick={(event) => {
          event.stopPropagation()
          player.toggle(track)
        }}
      >
        {playing ? <PauseGlyph /> : <PlayGlyph />}
      </button>
      <div className="tg-voice__body">
        <VoiceWaveform
          waveform={waveform}
          bars={waveformBarCount(track.durationMs)}
          progress={progress}
          durationMs={track.durationMs}
          onSeek={(fraction) => player.seek(track, fraction)}
        />
        <div className="tg-voice__info">
          <span className="tg-voice__time">{formatVoiceDuration(shown)}</span>
          {listened ? null : <span className="tg-voice__unlistened" role="img" aria-label="未收听" />}
          {loaded ? (
            <button
              type="button"
              className="tg-voice__rate"
              aria-label={`播放速度 ${rate}x，点击切换`}
              onClick={(event) => {
                event.stopPropagation()
                player.cycleRate()
              }}
            >
              {rate}x
            </button>
          ) : null}
          {metaSpacer}
        </div>
      </div>
    </div>
  )
}
