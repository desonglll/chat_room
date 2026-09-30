// TG-401: the voice client against an injected fake fetch — path, multipart fields, errors.
import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from './http'
import { markVoiceListened, sendVoiceMessage, VOICE_RESTRICTED, VOICE_WAVEFORM_SAMPLES } from './voice'

const waveform = Array.from({ length: VOICE_WAVEFORM_SAMPLES }, (_, index) => index % 32)

describe('voice api', () => {
  test('sends the recording as multipart with the wire field names', async () => {
    const calls: Array<{ url: string; init: RequestInit | undefined }> = []
    const stored = await sendVoiceMessage(
      async (url, init) => {
        calls.push({ url, init })
        return Response.json({ id: 'm1', voice: { duration_ms: 1200, waveform, listened: false } }, { status: 201 })
      },
      {
        chatId: 'chat/1',
        token: 't',
        audio: new Blob([new Uint8Array([1, 2, 3])], { type: 'audio/webm' }),
        fileName: 'voice.webm',
        durationMs: 1199.6,
        waveform,
        replyTo: 'r1',
      },
    )
    expect(stored.id).toBe('m1')
    expect(calls[0]!.url).toBe('/api/chats/chat%2F1/voice')
    expect(calls[0]!.init?.method).toBe('POST')
    expect((calls[0]!.init?.headers as Record<string, string>).Authorization).toBe('Bearer t')
    const form = calls[0]!.init?.body as FormData
    expect(form.get('waveform')).toBe(waveform.join(','))
    expect(form.get('duration_ms')).toBe('1200')
    expect(form.get('reply_to')).toBe('r1')
    const file = form.get('file') as File
    expect(file.name).toBe('voice.webm')
    expect(file.size).toBe(3)
  })

  test('surfaces the server error code, e.g. the privacy refusal', async () => {
    const attempt = sendVoiceMessage(async () => Response.json({ error: VOICE_RESTRICTED }, { status: 403 }), {
      chatId: 'c',
      token: 't',
      audio: new Blob([new Uint8Array([1])]),
      fileName: 'voice.ogg',
      durationMs: 10,
      waveform,
    })
    const error = await attempt.catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).status).toBe(403)
    expect((error as ApiError).serverMessage).toBe(VOICE_RESTRICTED)
  })

  test('marks a message listened by its id', async () => {
    const calls: Array<{ url: string; method: string | undefined }> = []
    const client = createApiClient({
      fetchImpl: async (url, init) => {
        calls.push({ url, method: init?.method })
        return new Response(null, { status: 204 })
      },
    })
    await markVoiceListened(client, 't', 'm 1')
    expect(calls).toEqual([{ url: '/api/messages/m%201/voice/listened', method: 'POST' }])
  })
})
