import { describe, expect, test } from 'bun:test'
import { videoNoteErrorText } from '../../videoNote/videoNoteErrors'
import { recordErrorText } from '../recordController'
import { mediaFailure, RecorderError, type RecorderFailure, unavailableFailure } from '../voiceRecorder'

const domError = (name: string) => Object.assign(new Error(name), { name })

describe('TG-1301 recorder failures', () => {
  test('no recording API on an insecure page is "insecure", not "this browser cannot"', () => {
    expect(unavailableFailure({ isSecureContext: false })).toBe('insecure')
    expect(unavailableFailure({ isSecureContext: true })).toBe('unsupported')
    expect(unavailableFailure({})).toBe('unsupported')
  })

  test('getUserMedia rejections split into permission, no device, and other device trouble', () => {
    for (const name of ['NotAllowedError', 'SecurityError', 'PermissionDeniedError']) {
      expect(mediaFailure(domError(name))).toBe('permission')
    }
    for (const name of ['NotFoundError', 'DevicesNotFoundError', 'OverconstrainedError']) {
      expect(mediaFailure(domError(name))).toBe('nodevice')
    }
    expect(mediaFailure(domError('NotReadableError'))).toBe('device')
    expect(mediaFailure(null)).toBe('device')
  })

  test('every failure has distinct copy for voice and for round video; the insecure copy names HTTPS', () => {
    const reasons: RecorderFailure[] = ['insecure', 'unsupported', 'permission', 'nodevice', 'device']
    for (const text of [recordErrorText, videoNoteErrorText]) {
      const copy = reasons.map((reason) => text(new RecorderError(reason)))
      expect(new Set(copy).size).toBe(reasons.length)
      expect(copy[0]).toContain('HTTPS')
    }
    expect(recordErrorText(new RecorderError('nodevice'))).toBe('没有找到麦克风')
    expect(videoNoteErrorText(new RecorderError('nodevice'))).toBe('没有找到摄像头')
  })
})
