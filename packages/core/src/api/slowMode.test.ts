import { describe, expect, test } from 'bun:test'
import { SLOW_MODE_CHOICES, slowModeCountdown, slowModeLabel } from './slowMode'

describe('slow mode copy', () => {
  test('every choice has a Telegram-style label', () => {
    expect(SLOW_MODE_CHOICES.map(slowModeLabel)).toEqual([
      '关闭',
      '10 秒',
      '30 秒',
      '1 分钟',
      '5 分钟',
      '15 分钟',
      '1 小时',
    ])
  })

  test('the countdown is m:ss and never negative', () => {
    expect(slowModeCountdown(42)).toBe('0:42')
    expect(slowModeCountdown(245)).toBe('4:05')
    expect(slowModeCountdown(3599)).toBe('59:59')
    expect(slowModeCountdown(0.2)).toBe('0:01')
    expect(slowModeCountdown(-3)).toBe('0:00')
  })
})
