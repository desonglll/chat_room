/**
 * The creation form's state and validation, pure so it is tested without a DOM. Limits come
 * from `POLL_LIMITS` (mirroring the server), and the server re-validates everything.
 */
import type { CreatePollInput } from '@tg/core'
import { POLL_LIMITS } from '@tg/core'

export interface PollForm {
  question: string
  options: string[]
  anonymous: boolean
  multipleChoice: boolean
  quiz: boolean
  /** Index into `options` (the raw list, blanks included). */
  correctOption: number | null
  explanation: string
}

export const EMPTY_POLL_FORM: PollForm = {
  question: '',
  options: ['', ''],
  anonymous: true,
  multipleChoice: false,
  quiz: false,
  correctOption: null,
  explanation: '',
}

const length = (text: string) => [...text.trim()].length

export function canAddOption(form: PollForm): boolean {
  return form.options.length < POLL_LIMITS.maxOptions
}

export function addOption(form: PollForm): PollForm {
  return canAddOption(form) ? { ...form, options: [...form.options, ''] } : form
}

export function removeOption(form: PollForm, index: number): PollForm {
  if (form.options.length <= POLL_LIMITS.minOptions) return form
  const options = form.options.filter((_, at) => at !== index)
  let correctOption = form.correctOption
  if (correctOption === index) correctOption = null
  else if (correctOption !== null && correctOption > index) correctOption -= 1
  return { ...form, options, correctOption }
}

/** Quiz and multiple choice exclude each other, as in Telegram. */
export function setQuiz(form: PollForm, quiz: boolean): PollForm {
  return { ...form, quiz, multipleChoice: quiz ? false : form.multipleChoice }
}

export function setMultipleChoice(form: PollForm, multipleChoice: boolean): PollForm {
  return { ...form, multipleChoice, quiz: multipleChoice ? false : form.quiz }
}

export type PollFormResult = { ok: true; input: CreatePollInput } | { ok: false; error: string }

/** Blank options are dropped (Telegram ignores the trailing empty row). */
export function buildPollInput(form: PollForm): PollFormResult {
  const question = form.question.trim()
  if (!question) return { ok: false, error: '请输入问题' }
  if (length(question) > POLL_LIMITS.questionChars) {
    return { ok: false, error: `问题不能超过 ${POLL_LIMITS.questionChars} 个字符` }
  }
  const kept = form.options.map((text, index) => ({ text: text.trim(), index })).filter((option) => option.text)
  if (kept.length < POLL_LIMITS.minOptions) return { ok: false, error: '至少需要两个选项' }
  if (kept.some((option) => length(option.text) > POLL_LIMITS.optionChars)) {
    return { ok: false, error: `选项不能超过 ${POLL_LIMITS.optionChars} 个字符` }
  }
  const input: CreatePollInput = {
    question,
    options: kept.map((option) => option.text),
    public_voters: !form.anonymous,
    multiple_choice: form.multipleChoice,
    quiz: form.quiz,
  }
  if (form.quiz) {
    const correct = kept.findIndex((option) => option.index === form.correctOption)
    if (correct < 0) return { ok: false, error: '请选择正确答案' }
    input.correct_option = correct
    const explanation = form.explanation.trim()
    if (length(explanation) > POLL_LIMITS.explanationChars) {
      return { ok: false, error: `解析不能超过 ${POLL_LIMITS.explanationChars} 个字符` }
    }
    if (explanation) input.explanation = explanation
  }
  return { ok: true, input }
}
