// TG-402: the video note client against an injected fake fetch; playback exclusivity and
// watched reporting.
import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from '@tg/core'
import { createVoiceStore } from '../../voice/voiceStore'
import { markVideoNoteListened, sendVideoNote } from '../videoNoteApi'
import { createVideoNotePlayback, reportVideoNoteWatched } from '../videoNotePlayback'

describe('video note api', () => {
  test('sends the recording, its duration and thumbnail as multipart', async () => {
    const calls: Array<{ url: string; init: RequestInit | undefined }> = []
    const stored = await sendVideoNote(
      async (url, init) => {
        calls.push({ url, init })
        return Response.json({ id: 'm1' }, { status: 201 })
      },
      {
        chatId: 'chat/1',
        token: 't',
        video: new Blob([new Uint8Array([1, 2, 3, 4])], { type: 'video/webm' }),
        fileName: 'video_note.webm',
        durationMs: 5_999.6,
        thumbnail: new Blob([new Uint8Array([0xff, 0xd8, 0xff])], { type: 'image/jpeg' }),
        replyTo: 'r1',
      },
    )
    expect(stored.id).toBe('m1')
    expect(calls[0]!.url).toBe('/api/chats/chat%2F1/video_note')
    expect((calls[0]!.init?.headers as Record<string, string>).Authorization).toBe('Bearer t')
    const form = calls[0]!.init?.body as FormData
    expect(form.get('duration_ms')).toBe('6000')
    expect(form.get('reply_to')).toBe('r1')
    expect((form.get('file') as File).name).toBe('video_note.webm')
    expect((form.get('thumbnail') as File).size).toBe(3)
  })

  test('no thumbnail, no field; server errors keep their code', async () => {
    let form: FormData | null = null
    const attempt = sendVideoNote(
      async (_url, init) => {
        form = init?.body as FormData
        return Response.json({ error: 'unsupported_video' }, { status: 415 })
      },
      { chatId: 'c', token: 't', video: new Blob([]), fileName: 'video_note.mp4', durationMs: 10 },
    )
    const error = await attempt.catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).serverMessage).toBe('unsupported_video')
    expect(form!.has('thumbnail')).toBe(false)
  })

  test('marks a note watched by its id', async () => {
    const calls: string[] = []
    const client = createApiClient({
      fetchImpl: async (url, init) => {
        calls.push(`${init?.method} ${url}`)
        return new Response(null, { status: 204 })
      },
    })
    await markVideoNoteListened(client, 't', 'm 1')
    expect(calls).toEqual(['POST /api/messages/m%201/video_note/listened'])
  })
})

describe('playback', () => {
  test('one video note sounds at a time; a stale deactivate is ignored', () => {
    const playback = createVideoNotePlayback()
    playback.getState().activate('a')
    playback.getState().activate('b')
    expect(playback.getState().active).toBe('b')
    playback.getState().deactivate('a')
    expect(playback.getState().active).toBe('b')
    playback.getState().deactivate('b')
    expect(playback.getState().active).toBeNull()
  })

  test('watching clears the dot at once and tells the server once', async () => {
    const calls: string[] = []
    const client = createApiClient({
      fetchImpl: async (url) => {
        calls.push(url)
        return new Response(null, { status: 204 })
      },
    })
    const store = createVoiceStore()
    reportVideoNoteWatched('w1', { client, token: 't', store })
    reportVideoNoteWatched('w1', { client, token: 't', store })
    expect(store.getState().listened.w1).toBe(true)
    await Promise.resolve()
    expect(calls).toEqual(['/api/messages/w1/video_note/listened'])
  })
})
