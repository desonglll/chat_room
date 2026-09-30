// TG-401: listened state — frames through the real chat session, local marks, reporting once.
import { describe, expect, test } from 'bun:test'
import type { ApiClient } from '@tg/core'
import { selectTimeline } from '@tg/core'
import { broadcastFrame, CHAT_ID, harness, ME } from '../../../../test/chatSessionHarness'
import { applyVoiceListenedFrame, createVoiceStore, reportListened, voiceStore } from '../voiceStore'

const VOICE = { duration_ms: 2_000, waveform: new Array(100).fill(5), listened: false }
const AUDIO = {
  id: 'att-1',
  file_name: 'voice.webm',
  mime_type: 'audio/webm',
  size_bytes: 10,
  download_url: '/api/attachments/att-1?key=k',
  is_sensitive: false,
}

describe('listened state', () => {
  test('a voice_listened frame on the chat socket clears the dot (sender side)', async () => {
    voiceStore.getState().clear()
    const { session, sockets, stores, online } = harness()
    await online()
    sockets[0]!.receive(
      broadcastFrame('v1', '2026-10-01T09:00:00Z', '', { sender_id: ME, attachment: AUDIO, voice: VOICE }),
    )
    const [row] = selectTimeline(CHAT_ID)(stores.message.getState()).messages
    expect(row).toMatchObject({ message_id: 'v1', voice: { listened: false } })
    expect(voiceStore.getState().listened.v1).toBeUndefined()
    sockets[0]!.receive({ type: 'voice_listened', message_id: 'v1', user_id: 'user-other', sender_id: ME })
    expect(voiceStore.getState().listened.v1).toBeTrue()
    session.stop()
  })

  test('frames and local marks are idempotent', () => {
    const store = createVoiceStore()
    applyVoiceListenedFrame({ type: 'voice_listened', message_id: 'm', user_id: 'u', sender_id: 's' }, store)
    const first = store.getState()
    store.getState().markListened('m')
    expect(store.getState()).toBe(first)
  })

  test('the viewer playing a message reports it once, and retries after a failure', async () => {
    const calls: string[] = []
    let fail = true
    const client = {
      request: async (_method: string, path: string) => {
        calls.push(path)
        if (fail) throw new Error('offline')
        return new Response(null, { status: 204 })
      },
    } as unknown as ApiClient
    const store = createVoiceStore()
    reportListened('r1', { client, token: 't', store })
    expect(store.getState().listened.r1).toBeTrue()
    await Promise.resolve()
    await Promise.resolve()
    fail = false
    reportListened('r1', { client, token: 't', store })
    reportListened('r1', { client, token: 't', store })
    expect(calls).toEqual(['/api/messages/r1/voice/listened', '/api/messages/r1/voice/listened'])
  })
})
