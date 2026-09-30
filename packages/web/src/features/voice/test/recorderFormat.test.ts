// TG-401: container choice, including Safari's AAC/MP4 fallback.
import { describe, expect, test } from 'bun:test'
import { containerOf, pickRecorderFormat, uploadMimeType, voiceFileName } from '../recorderFormat'

const supports =
  (...types: string[]) =>
  (type: string) =>
    types.includes(type)

describe('pickRecorderFormat', () => {
  test('Chromium records Opus in WebM', () => {
    expect(pickRecorderFormat(supports('audio/webm;codecs=opus', 'audio/mp4'))).toEqual({
      mimeType: 'audio/webm;codecs=opus',
      container: 'webm',
    })
  })

  test('an Ogg-only recorder records Opus in Ogg', () => {
    expect(pickRecorderFormat(supports('audio/ogg;codecs=opus'))?.container).toBe('ogg')
  })

  test('Safari (no Opus) falls back to AAC in MP4', () => {
    expect(pickRecorderFormat(supports('audio/mp4;codecs=mp4a.40.2', 'audio/mp4'))).toEqual({
      mimeType: 'audio/mp4;codecs=mp4a.40.2',
      container: 'mp4',
    })
    expect(pickRecorderFormat(supports('audio/mp4'))?.mimeType).toBe('audio/mp4')
  })

  test('no supported type, or no detection at all, means no recording', () => {
    expect(pickRecorderFormat(supports())).toBeNull()
    expect(pickRecorderFormat(undefined)).toBeNull()
  })
})

describe('containers', () => {
  test('the finished blob type wins over the requested one', () => {
    expect(containerOf('audio/webm;codecs=opus', 'mp4')).toBe('webm')
    expect(containerOf('audio/mp4', 'webm')).toBe('mp4')
    expect(containerOf('', 'ogg')).toBe('ogg')
  })

  test('file names and upload types', () => {
    expect(voiceFileName('mp4')).toBe('voice.m4a')
    expect(voiceFileName('ogg')).toBe('voice.ogg')
    expect(uploadMimeType('webm')).toBe('audio/webm')
  })
})
