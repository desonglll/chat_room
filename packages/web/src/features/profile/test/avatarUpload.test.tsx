/** TG-1204: a profile photo can be uploaded at all (the React client had no upload). */
import { afterEach, describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { AuthSession } from '@tg/core'
import { ApiError, authStore } from '@tg/core'
import { AvatarUpload } from '../AvatarCarousel'
import { profileApi } from '../profileApi'

const session = { token: 'tok', user: { id: 'me', username: 'me' }, expires_at: '2099-01-01T00:00:00Z' } as AuthSession

afterEach(() => authStore.getState().clearSession())

describe('TG-1204 avatar upload', () => {
  test('posts the image as multipart `file` with the session token', async () => {
    authStore.getState().setSession(session)
    const calls: { input: string; init: RequestInit }[] = []
    const user = await profileApi.upload(
      new File([new Uint8Array([1, 2])], 'me.png', { type: 'image/png' }),
      async (input, init) => {
        calls.push({ input, init })
        return new Response(JSON.stringify({ id: 'me', username: 'me' }), { status: 200 })
      },
    )
    expect(user.id).toBe('me')
    expect(calls[0]!.input).toBe('/api/users/me/avatar')
    expect(calls[0]!.init.method).toBe('POST')
    expect((calls[0]!.init.headers as Record<string, string>).Authorization).toBe('Bearer tok')
    const file = (calls[0]!.init.body as FormData).get('file') as File
    expect(file.name).toBe('me.png')
  })

  test('a refused upload surfaces its status', async () => {
    const refused = profileApi.upload(new Blob(['x']), async () => new Response('', { status: 413 }))
    await expect(refused).rejects.toBeInstanceOf(ApiError)
    await expect(refused).rejects.toMatchObject({ status: 413 })
  })

  test('the upload control is a labelled image file input and shows the last failure', () => {
    const idle = renderToStaticMarkup(<AvatarUpload busy={false} failure="" onFile={() => {}} />)
    expect(idle).toContain('上传新头像')
    expect(idle).toMatch(/<input type="file" accept="image\/png,[^"]*"/)
    expect(idle).not.toContain('role="alert"')
    const failed = renderToStaticMarkup(<AvatarUpload busy failure="上传失败" onFile={() => {}} />)
    expect(failed).toContain('aria-disabled="true"')
    expect(failed).toContain('<p role="alert">上传失败</p>')
  })
})
