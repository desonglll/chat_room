/**
 * Radio hit-testing (TG-110). The ring is `position: relative` and comes after the invisible
 * native input, so by default it paints above the input and swallows every click on the circle
 * of a radio without a visible `<label>` (found by TG-406 in the quiz editor). No DOM renderer
 * is installed, so this pins the two halves of the fix: the markup order the CSS relies on, and
 * the stacking rules themselves.
 */
import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { Radio } from './Radio'

const css = readFileSync(new URL('./Radio.css', import.meta.url), 'utf8')

function rule(selector: string): string {
  const start = css.indexOf(`${selector} {`)
  expect(start).toBeGreaterThanOrEqual(0)
  return css.slice(start, css.indexOf('}', start))
}

test('a label-less radio renders the native input before its painted ring', () => {
  const html = renderToStaticMarkup(<Radio value="a" name="g" />)
  const input = html.indexOf('class="tg-radio__input"')
  const ring = html.indexOf('class="tg-radio__dot"')
  expect(input).toBeGreaterThanOrEqual(0)
  expect(ring).toBeGreaterThan(input)
  expect(html).not.toContain('<label')
})

test('the native input is stacked above the ring and the ring ignores pointer events', () => {
  expect(rule('.tg-radio__input')).toMatch(/position:\s*absolute/)
  expect(rule('.tg-radio__input')).toMatch(/z-index:\s*1/)
  expect(rule('.tg-radio__dot')).toMatch(/pointer-events:\s*none/)
})
