/**
 * TG-1204: the appearance page's colour inputs start at the current accent. Lives outside `src`
 * because it needs colour literals, which the token-discipline scan forbids there.
 */
import { describe, expect, test } from 'bun:test'
import { toHexColor } from '../src/features/settings/appearance/cssColor'

describe('TG-1204 toHexColor', () => {
  test('a minified or rgb() accent still becomes a value <input type="color"> accepts', () => {
    expect(toHexColor('#08f')).toBe('#0088ff')
    expect(toHexColor(' #3390EC ')).toBe('#3390ec')
    expect(toHexColor('#3390ec80')).toBe('#3390ec')
    expect(toHexColor('#08f8')).toBe('#0088ff')
    expect(toHexColor('rgb(51, 144, 236)')).toBe('#3390ec')
    expect(toHexColor('rgba(51 144 236 / 50%)')).toBe('#3390ec')
    expect(toHexColor('var(--x)')).toBe('')
  })
})
