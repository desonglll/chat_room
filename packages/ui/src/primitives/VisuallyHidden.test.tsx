import { expect, test } from 'bun:test'
import { VisuallyHidden } from './VisuallyHidden'

// No DOM renderer is installed yet (that is a TG-010 decision), so this asserts on the element
// the component returns. It is enough to prove the JSX pipeline and the package entry work.
test('VisuallyHidden returns a span that is hidden visually but not from assistive tech', () => {
  const element = VisuallyHidden({ children: 'skip to message list' })

  expect(element.type).toBe('span')
  expect(element.props.children).toBe('skip to message list')
  expect(element.props.style).toMatchObject({ position: 'absolute', clipPath: 'inset(50%)' })
  expect(element.props).not.toHaveProperty('aria-hidden')
})
