/**
 * The browser implementation of `RecorderEnv`: getUserMedia, MediaRecorder and an
 * AnalyserNode level tap. The only file of this feature that names recording globals.
 */
import type { LevelTap, MediaStreamLike, RecorderEnv } from './voiceRecorder'

type AudioContextCtor = typeof AudioContext

function audioContextCtor(): AudioContextCtor | undefined {
  const scope = window as unknown as { AudioContext?: AudioContextCtor; webkitAudioContext?: AudioContextCtor }
  return scope.AudioContext ?? scope.webkitAudioContext
}

function createLevelTap(stream: MediaStreamLike): LevelTap | null {
  const Ctor = audioContextCtor()
  if (!Ctor) return null
  try {
    const context = new Ctor()
    const source = context.createMediaStreamSource(stream as MediaStream)
    const analyser = context.createAnalyser()
    analyser.fftSize = 1024
    source.connect(analyser)
    const buffer = new Float32Array(analyser.fftSize)
    return {
      read() {
        analyser.getFloatTimeDomainData(buffer)
        let peak = 0
        for (const value of buffer) peak = Math.max(peak, Math.abs(value))
        return peak
      },
      close() {
        source.disconnect()
        void context.close().catch(() => {})
      },
    }
  } catch {
    return null
  }
}

/** `undefined` when this browser has no MediaRecorder or no microphone API at all. */
export function browserRecorderEnv(): RecorderEnv | undefined {
  if (typeof MediaRecorder === 'undefined' || !navigator.mediaDevices?.getUserMedia) return undefined
  return {
    isTypeSupported:
      typeof MediaRecorder.isTypeSupported === 'function' ? (type) => MediaRecorder.isTypeSupported(type) : undefined,
    getUserMedia: () =>
      navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } }),
    createRecorder: (stream, mimeType) =>
      new MediaRecorder(stream as MediaStream, { mimeType, audioBitsPerSecond: 32_000 }) as unknown as ReturnType<
        RecorderEnv['createRecorder']
      >,
    createLevelTap,
    now: () => performance.now(),
    setInterval: (callback, ms) => window.setInterval(callback, ms),
    clearInterval: (handle) => window.clearInterval(handle as number),
  }
}
