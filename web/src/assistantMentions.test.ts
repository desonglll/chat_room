import { describe, expect, test } from 'bun:test'
import { parseAssistantPrompt } from './assistantMentions'

const conversations = [
  { roomId: 'room-1', title: '项目 Alpha' },
  { roomId: 'room-2', title: '设计讨论' },
]

describe('AI assistant mentions', () => {
  test('resolves the longest matching conversation mention and strips assistant mention', () => {
    expect(parseAssistantPrompt('@AI助手 @项目 Alpha 总结一下', conversations)).toEqual({
      roomId: 'room-1',
      question: '总结一下',
    })
  })

  test('uses an already selected room when the prompt has no room mention', () => {
    expect(parseAssistantPrompt('@AI助手 有哪些待办？', conversations, 'room-2')).toEqual({
      roomId: 'room-2',
      question: '有哪些待办？',
    })
  })
})
