export interface CalculatorKeyEvent {
  key: string
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
  ctrlKey: boolean
  isComposing: boolean
  keyCode: number
}

export type CalculationResult = { ok: true; value: string } | { ok: false; error: string }

type CalculatorErrorReason = 'division-zero' | 'invalid' | 'overflow'

class CalculatorError extends Error {
  constructor(readonly reason: CalculatorErrorReason) {
    super(reason)
  }
}

class ArithmeticParser {
  private index = 0

  constructor(private readonly expression: string) {}

  parse(): number {
    if (!this.expression.trim()) throw new CalculatorError('invalid')
    const value = this.parseSum()
    this.skipWhitespace()
    if (this.index !== this.expression.length) throw new CalculatorError('invalid')
    return value
  }

  private parseSum(): number {
    let value = this.parseProduct()
    while (true) {
      if (this.match('+')) value += this.parseProduct()
      else if (this.match('-')) value -= this.parseProduct()
      else return value
    }
  }

  private parseProduct(): number {
    let value = this.parseUnary()
    while (true) {
      if (this.match('*')) {
        value *= this.parseUnary()
      } else if (this.match('/')) {
        const divisor = this.parseUnary()
        if (divisor === 0) throw new CalculatorError('division-zero')
        value /= divisor
      } else {
        return value
      }
    }
  }

  private parseUnary(): number {
    if (this.match('+')) return this.parseUnary()
    if (this.match('-')) return -this.parseUnary()
    return this.parsePrimary()
  }

  private parsePrimary(): number {
    if (this.match('(')) {
      const value = this.parseSum()
      if (!this.match(')')) throw new CalculatorError('invalid')
      return value
    }

    this.skipWhitespace()
    const number = this.expression.slice(this.index).match(/^(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?/i)?.[0]
    if (!number) throw new CalculatorError('invalid')
    this.index += number.length
    return Number(number)
  }

  private match(token: string): boolean {
    this.skipWhitespace()
    if (this.expression[this.index] !== token) return false
    this.index += 1
    return true
  }

  private skipWhitespace(): void {
    while (/\s/.test(this.expression[this.index] || '')) this.index += 1
  }
}

function normalizeExpression(expression: string): string {
  return expression.replaceAll('×', '*').replaceAll('÷', '/').replaceAll('（', '(').replaceAll('）', ')')
}

function formatResult(value: number): string {
  if (!Number.isFinite(value)) throw new CalculatorError('overflow')
  if (Object.is(value, -0)) return '0'
  if (Number.isSafeInteger(value)) return String(value)
  return String(Number.parseFloat(value.toPrecision(15)))
}

export function evaluateArithmeticExpression(expression: string): CalculationResult {
  try {
    return { ok: true, value: formatResult(new ArithmeticParser(normalizeExpression(expression)).parse()) }
  } catch (error) {
    if (error instanceof CalculatorError && error.reason === 'division-zero') {
      return { ok: false, error: '不能除以 0' }
    }
    if (error instanceof CalculatorError && error.reason === 'overflow') {
      return { ok: false, error: '结果超出可计算范围' }
    }
    return { ok: false, error: '无法计算这个算式' }
  }
}

export function shouldCalculateExpression(event: CalculatorKeyEvent, composing: boolean): boolean {
  return (
    event.key === 'Enter' &&
    event.altKey &&
    !event.shiftKey &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.isComposing &&
    !composing &&
    event.keyCode !== 229
  )
}
