// TG-204: the create / edit topic form — validation, create body, minimal edit patch.
import { expect, test } from 'bun:test'
import { MAX_TOPIC_TITLE_CHARS, TOPIC_COLORS } from '@tg/core'
import { makeTopic } from './topicFixtures'
import { initialTopicForm, topicCreateInput, topicPatch, validateTopicForm } from './topicForm'

test('a new form picks a palette colour from the seed and starts empty', () => {
  expect(initialTopicForm(null, 0)).toEqual({ title: '', emoji: '', color: TOPIC_COLORS[0] })
  expect(initialTopicForm(null, 0.99).color).toBe(TOPIC_COLORS[5])
  expect(initialTopicForm(makeTopic('a', { icon_emoji: '🔥' })).emoji).toBe('🔥')
})

test('title is required, trimmed and at most 128 characters (code points, not UTF-16 units)', () => {
  const base = { emoji: '', color: TOPIC_COLORS[1] }
  expect(validateTopicForm({ ...base, title: '   ' })).toBe('请输入话题名称')
  expect(validateTopicForm({ ...base, title: 'x'.repeat(MAX_TOPIC_TITLE_CHARS) })).toBeNull()
  expect(validateTopicForm({ ...base, title: '😀'.repeat(MAX_TOPIC_TITLE_CHARS) })).toBeNull()
  expect(validateTopicForm({ ...base, title: 'x'.repeat(MAX_TOPIC_TITLE_CHARS + 1) })).toContain('128')
  expect(validateTopicForm({ ...base, title: 'ok', color: 0x123456 })).toBe('请选择话题颜色')
})

test('the create body trims the title and omits an empty emoji', () => {
  expect(topicCreateInput({ title: ' 公告 ', emoji: '', color: TOPIC_COLORS[2] })).toEqual({
    title: '公告',
    icon_color: TOPIC_COLORS[2],
  })
  expect(topicCreateInput({ title: 'a', emoji: '📌', color: TOPIC_COLORS[0] }).icon_emoji).toBe('📌')
})

test('an edit sends only what changed; General edits its title only', () => {
  const topic = makeTopic('a', { title: '旧', icon_emoji: '🔥' })
  expect(topicPatch({ title: '旧', emoji: '🔥', color: topic.icon_color }, topic)).toEqual({})
  expect(topicPatch({ title: '新', emoji: '', color: TOPIC_COLORS[3] }, topic)).toEqual({
    title: '新',
    icon_emoji: '',
    icon_color: TOPIC_COLORS[3],
  })
  const general = makeTopic('g', { is_general: true, title: 'General' })
  expect(topicPatch({ title: '综合', emoji: '🔥', color: TOPIC_COLORS[4] }, general)).toEqual({ title: '综合' })
})
