import { describe, expect, test } from 'bun:test'
import {
  aiContextUsage,
  aiRunStageStatus,
  aiSourceRoute,
  citedAiSources,
  formatLocalDateTime,
  inlineAiAttachments,
  referencedAiAttachments,
  trailingAiAttachments,
} from './aiUi'

const source = {
  label: 'S1',
  room_id: 'room-1',
  message_id: 'message-9',
  sender: 'Ada',
  sent_at: '2026-08-25T10:00:00Z',
  excerpt: 'The launch date is Friday',
  score: 0.82,
}

describe('AI context usage', () => {
  test('separates recent context from full-room RAG matches', () => {
    expect(aiContextUsage(130, 12)).toEqual({ recent: 118, retrieved: 12 })
  })

  test('does not claim RAG when no semantic evidence was injected', () => {
    expect(aiContextUsage(118, null)).toEqual({ recent: 118, retrieved: 0 })
  })

  test('links a RAG source to the original room message', () => {
    expect(aiSourceRoute(source)).toEqual({
      name: 'room',
      params: { id: 'room-1' },
      query: { message: 'message-9' },
      hash: '#message-message-9',
    })
  })

  test('keeps only sources explicitly cited by the answer', () => {
    const sources = [source, { ...source, label: 'S2', message_id: 'message-10' }]
    expect(citedAiSources('结论来自 [S2]。', sources)).toEqual([sources[1]])
    expect(citedAiSources('没有引用标签', sources)).toEqual([])
  })

  test('finds attachments cited by label or by their exact file name', () => {
    const imageSource = {
      ...source,
      label: 'A1',
      score_kind: 'attachment',
      attachment: {
        id: 'attachment-1',
        file_name: 'design-review.png',
        mime_type: 'image/png',
        size_bytes: 1200,
        download_url: '/api/attachments/attachment-1?key=access',
        is_sensitive: false,
      },
    }
    expect(referencedAiAttachments('参见图片 [A1]', [imageSource])).toEqual([imageSource])
    expect(referencedAiAttachments('我建议打开 design-review.png', [imageSource])).toEqual([imageSource])
    expect(referencedAiAttachments('没有提到附件', [imageSource])).toEqual([])
    expect(referencedAiAttachments('参见 [A1] 和 [S1]', [imageSource, { ...imageSource, label: 'S1' }])).toEqual([
      imageSource,
    ])
    expect(inlineAiAttachments('参见图片 [A1]', [imageSource])).toEqual([imageSource])
    expect(trailingAiAttachments('参见图片 [A1]', [imageSource])).toEqual([])
    expect(inlineAiAttachments('我建议打开 design-review.png', [imageSource])).toEqual([])
    expect(trailingAiAttachments('我建议打开 design-review.png', [imageSource])).toEqual([imageSource])
  })

  test('keeps inline attachments in citation order and removes duplicates', () => {
    const attachment = {
      id: 'attachment-1',
      file_name: 'design-review.png',
      mime_type: 'image/png',
      size_bytes: 1200,
      download_url: '/api/attachments/attachment-1?key=access',
      is_sensitive: false,
    }
    const sources = [
      { ...source, label: 'A1', message_id: 'message-1', attachment },
      { ...source, label: 'A2', message_id: 'message-2', attachment: { ...attachment, id: 'attachment-2' } },
    ]
    expect(inlineAiAttachments('先看 [A2]，再看 [A1]，不要重复 [A2]。', sources)).toEqual([sources[1], sources[0]])
  })

  test('renders UTC and legacy offsetless values in the selected user timezone', () => {
    expect(formatLocalDateTime('2026-08-25T10:00:00Z', 'Asia/Shanghai')).toContain('18:00')
    expect(formatLocalDateTime('2026-08-25 10:00:00', 'America/New_York')).toContain('06:00')
  })

  test('describes the active backend stage with stage and total elapsed time', () => {
    expect(
      aiRunStageStatus(
        {
          stage: 'waiting_for_model',
          stage_started_at: '2026-08-26T10:00:04Z',
          created_at: '2026-08-26T10:00:00Z',
        },
        Date.parse('2026-08-26T10:00:11Z'),
      ),
    ).toEqual({ label: '等待模型首个响应', stageSeconds: 7, totalSeconds: 11 })
  })
})
