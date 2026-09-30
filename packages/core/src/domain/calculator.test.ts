import { describe, expect, test } from 'bun:test'
import { evaluateArithmeticExpression, shouldCalculateExpression, type CalculatorKeyEvent } from './calculator'

function keyEvent(overrides: Partial<CalculatorKeyEvent> = {}): CalculatorKeyEvent {
  return {
    key: 'Enter',
    altKey: true,
    shiftKey: false,
    metaKey: false,
    ctrlKey: false,
    isComposing: false,
    keyCode: 13,
    ...overrides,
  }
}

describe('arithmetic expression calculator', () => {
  test.each([
    ['2 + 3 * 4', '14'],
    ['-(2 + 3) * 4', '-20'],
    ['0.1 + 0.2', '0.3'],
    ['6 ÷ 4', '1.5'],
    ['2 × （3 + 4）', '14'],
    ['1e3 + 2', '1002'],
  ])('evaluates %s', (expression, expected) => {
    expect(evaluateArithmeticExpression(expression)).toEqual({ ok: true, value: expected })
  })

  test('reports division by zero without returning a result', () => {
    expect(evaluateArithmeticExpression('8 / (3 - 3)')).toEqual({ ok: false, error: '不能除以 0' })
  })

  test.each(['', '2 +', '2 apples', '((2)'])('rejects invalid expression %p', (expression) => {
    expect(evaluateArithmeticExpression(expression)).toEqual({ ok: false, error: '无法计算这个算式' })
  })

  test('rejects results outside the finite number range', () => {
    expect(evaluateArithmeticExpression('1e308 * 10')).toEqual({ ok: false, error: '结果超出可计算范围' })
  })
})

describe('calculator shortcut', () => {
  test('uses Option+Enter without accepting extra modifiers', () => {
    expect(shouldCalculateExpression(keyEvent(), false)).toBe(true)
    expect(shouldCalculateExpression(keyEvent({ altKey: false }), false)).toBe(false)
    expect(shouldCalculateExpression(keyEvent({ shiftKey: true }), false)).toBe(false)
    expect(shouldCalculateExpression(keyEvent({ metaKey: true }), false)).toBe(false)
    expect(shouldCalculateExpression(keyEvent({ ctrlKey: true }), false)).toBe(false)
  })

  test('does not calculate during IME composition', () => {
    expect(shouldCalculateExpression(keyEvent({ isComposing: true }), false)).toBe(false)
    expect(shouldCalculateExpression(keyEvent({ keyCode: 229 }), false)).toBe(false)
    expect(shouldCalculateExpression(keyEvent(), true)).toBe(false)
  })
})
