import { describe, expect, test } from 'bun:test'
import { EMPTY_POLL_FORM, addOption, buildPollInput, removeOption, setMultipleChoice, setQuiz } from '../pollForm'

const form = (overrides = {}) => ({ ...EMPTY_POLL_FORM, question: '问题', options: ['甲', '乙'], ...overrides })

describe('poll form', () => {
  test('builds the wire input, anonymous by default, dropping blank options', () => {
    const result = buildPollInput(form({ options: [' 甲 ', '', '乙'] }))
    expect(result).toEqual({
      ok: true,
      input: { question: '问题', options: ['甲', '乙'], public_voters: false, multiple_choice: false, quiz: false },
    })
  })

  test('refuses a missing question and fewer than two options', () => {
    expect(buildPollInput(form({ question: '  ' })).ok).toBe(false)
    expect(buildPollInput(form({ options: ['甲', ' '] })).ok).toBe(false)
  })

  test('a quiz maps its correct option past dropped blanks and needs one', () => {
    const quiz = setQuiz(form({ options: ['', '甲', '乙'] }), true)
    expect(buildPollInput(quiz).ok).toBe(false)
    const result = buildPollInput({ ...quiz, correctOption: 2, explanation: ' 解析 ' })
    expect(result.ok && result.input.correct_option).toBe(1)
    expect(result.ok && result.input.explanation).toBe('解析')
  })

  test('quiz and multiple choice exclude each other', () => {
    expect(setQuiz(setMultipleChoice(form(), true), true).multipleChoice).toBe(false)
    expect(setMultipleChoice(setQuiz(form(), true), true).quiz).toBe(false)
  })

  test('options stay within 2..10 and the correct pick follows removals', () => {
    let state = form()
    for (let i = 0; i < 12; i += 1) state = addOption(state)
    expect(state.options.length).toBe(10)
    expect(removeOption(form(), 0).options.length).toBe(2)
    const shifted = removeOption({ ...form({ options: ['a', 'b', 'c'] }), correctOption: 2 }, 0)
    expect(shifted.correctOption).toBe(1)
    expect(removeOption({ ...form({ options: ['a', 'b', 'c'] }), correctOption: 0 }, 0).correctOption).toBe(null)
  })
})
