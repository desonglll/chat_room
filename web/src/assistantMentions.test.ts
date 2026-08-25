import { describe, expect, test } from 'bun:test'
import {
  activeConversationMention,
  conversationMentionCandidates,
  insertConversationMention,
  parseAssistantPrompt,
} from './assistantMentions'

const conversations = [
  { roomId: 'room-1', title: '项目 Alpha' },
  { roomId: 'room-2', title: '设计讨论' },
]

describe('AI assistant mentions', () => {
  test('resolves the conversation mention without requiring an assistant mention', () => {
    expect(parseAssistantPrompt('@项目 Alpha 总结一下', conversations)).toEqual({
      roomId: 'room-1',
      question: '总结一下',
    })
  })

  test('uses an already selected room when the prompt has no room mention', () => {
    expect(parseAssistantPrompt('有哪些待办？', conversations, 'room-2')).toEqual({
      roomId: 'room-2',
      question: '有哪些待办？',
    })
  })

  test('keeps a normal AI question valid without a conversation', () => {
    expect(parseAssistantPrompt('帮我写一份会议议程', conversations)).toEqual({
      roomId: '',
      question: '帮我写一份会议议程',
    })
  })

  test('opens candidates at the active @ token and filters by typed text', () => {
    const value = '请分析 @项目'
    const range = activeConversationMention(value, value.length, conversations)
    expect(range).toEqual({ start: 4, end: 7, query: '项目' })
    expect(conversationMentionCandidates(range, conversations)).toEqual([conversations[0]])
  })

  test('inserts the chosen conversation and places the caret after it', () => {
    const value = '请分析 @项'
    const range = activeConversationMention(value, value.length, conversations)!
    expect(insertConversationMention(value, range, conversations[0])).toEqual({
      value: '请分析 @项目 Alpha ',
      caret: 14,
    })
  })

  test('does not reopen suggestions immediately after a completed mention', () => {
    const value = '@设计讨论 '
    expect(activeConversationMention(value, value.length, conversations)).toBeNull()
  })

  test('keeps suggestions closed while writing after a selected conversation', () => {
    const value = '@项目 Alpha 请总结最近的结论'
    expect(activeConversationMention(value, value.length, conversations)).toBeNull()
  })
})
