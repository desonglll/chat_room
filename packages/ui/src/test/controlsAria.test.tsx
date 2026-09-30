import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { Button } from '../primitives/Button'
import { Checkbox } from '../primitives/Checkbox'
import { IconButton } from '../primitives/IconButton'
import { RadioGroup } from '../primitives/Radio'
import { Ripple } from '../primitives/Ripple'
import { TextField } from '../primitives/TextField'
import { Toggle } from '../primitives/Toggle'
import { byRole, find, openTags, textOfId } from './markup'

// ── Button ──────────────────────────────────────────────────────────────────────

test('Button is type=button by default, so it cannot submit a form by accident', () => {
  const html = renderToStaticMarkup(<Button>Send</Button>)
  expect(find(html, { name: 'button', attributes: { type: 'button' } })).toHaveLength(1)
  expect(html).toContain('tg-button--filled')
  expect(html).toContain('tg-button--md')
})

test('Button honours an explicit type', () => {
  const html = renderToStaticMarkup(<Button type="submit">Save</Button>)
  expect(find(html, { name: 'button', attributes: { type: 'submit' } })).toHaveLength(1)
})

test('a loading Button keeps its accessible name and announces itself busy', () => {
  const html = renderToStaticMarkup(<Button loading>Sending</Button>)
  const button = openTags(html)[0]
  expect(button?.attributes['aria-busy']).toBe('true')
  expect(html).toContain('Sending')
  // The label is hidden visually (CSS `visibility`), not removed, so the name survives.
  expect(html).toContain('tg-button__label')
})

test('a disabled Button renders no ripple, because there is nothing to confirm', () => {
  expect(renderToStaticMarkup(<Button disabled>Send</Button>)).not.toContain('tg-ripple')
  expect(renderToStaticMarkup(<Button>Send</Button>)).toContain('tg-ripple')
})

test('Button icons are aria-hidden, so they do not double the announced name', () => {
  const html = renderToStaticMarkup(
    <Button startIcon={<svg />} endIcon={<svg />}>
      Attach
    </Button>,
  )
  expect(find(html, { attributes: { class: 'tg-button__icon', 'aria-hidden': 'true' } })).toHaveLength(2)
})

// ── IconButton ──────────────────────────────────────────────────────────────────

test('IconButton names itself from `label` and mirrors it into title', () => {
  const html = renderToStaticMarkup(
    <IconButton label="Attach file">
      <svg />
    </IconButton>,
  )
  const button = openTags(html)[0]
  expect(button?.attributes['aria-label']).toBe('Attach file')
  expect(button?.attributes['title']).toBe('Attach file')
  expect(find(html, { attributes: { class: 'tg-icon-button__glyph', 'aria-hidden': 'true' } })).toHaveLength(1)
})

test('IconButton exposes a toggle state as aria-pressed, and omits it when not a toggle', () => {
  const on = renderToStaticMarkup(
    <IconButton label="Mute" selected>
      <svg />
    </IconButton>,
  )
  expect(openTags(on)[0]?.attributes['aria-pressed']).toBe('true')

  const plain = renderToStaticMarkup(
    <IconButton label="Search">
      <svg />
    </IconButton>,
  )
  expect(openTags(plain)[0]?.attributes['aria-pressed']).toBeUndefined()
})

// ── Ripple ──────────────────────────────────────────────────────────────────────

test('Ripple is hidden from assistive technology and paints nothing until pressed', () => {
  const html = renderToStaticMarkup(<Ripple />)
  const span = openTags(html)[0]
  expect(span?.attributes['aria-hidden']).toBe('true')
  expect(html).not.toContain('tg-ripple__wave')
})

// ── TextField ───────────────────────────────────────────────────────────────────

test('TextField wires its label to the input and forces the placeholder the float needs', () => {
  const html = renderToStaticMarkup(<TextField label="Chat title" />)
  const input = find(html, { name: 'input' })[0]
  const label = find(html, { name: 'label' })[0]
  expect(input?.attributes['id']).toBeDefined()
  expect(label?.attributes['for']).toBe(input?.attributes['id'] as string)
  // A single space: the floating-label CSS needs :placeholder-shown to test emptiness.
  expect(input?.attributes['placeholder']).toBe(' ')
})

test('TextField describes itself with the hint and switches to the error, which is announced', () => {
  const hinted = renderToStaticMarkup(<TextField label="Title" hint="Up to 128 characters" />)
  const hintedInput = find(hinted, { name: 'input' })[0]
  const describedBy = hintedInput?.attributes['aria-describedby'] as string
  expect(textOfId(hinted, describedBy)).toBe('Up to 128 characters')
  expect(hintedInput?.attributes['aria-invalid']).toBeUndefined()

  const broken = renderToStaticMarkup(<TextField label="Title" hint="ignored" error="Title is required" />)
  const brokenInput = find(broken, { name: 'input' })[0]
  expect(brokenInput?.attributes['aria-invalid']).toBe('true')
  expect(textOfId(broken, brokenInput?.attributes['aria-describedby'] as string)).toBe('Title is required')
  expect(byRole(broken, 'alert')).toHaveLength(1)
  expect(broken).not.toContain('ignored')
})

test('TextField renders a textarea when multiline, keeping the same label wiring', () => {
  const html = renderToStaticMarkup(<TextField label="About" multiline rows={4} />)
  const area = find(html, { name: 'textarea' })[0]
  expect(area).toBeDefined()
  expect(area?.attributes['rows']).toBe('4')
  expect(find(html, { name: 'label' })[0]?.attributes['for']).toBe(area?.attributes['id'] as string)
})

test("TextField's clear button has a name and only appears when there is something to clear", () => {
  const empty = renderToStaticMarkup(<TextField label="Search" clearable value="" onChange={() => {}} />)
  expect(empty).not.toContain('tg-field__clear')

  const filled = renderToStaticMarkup(<TextField label="Search" clearable value="ada" onChange={() => {}} />)
  const clear = find(filled, { attributes: { class: 'tg-field__clear' } })[0]
  expect(clear?.attributes['aria-label']).toBe('Clear')
})

// ── Toggle ──────────────────────────────────────────────────────────────────────

test('Toggle is a switch, not a checkbox, and reports its state', () => {
  const html = renderToStaticMarkup(<Toggle label="Notifications" checked />)
  const control = byRole(html, 'switch')[0]
  expect(control?.attributes['aria-checked']).toBe('true')
  expect(control?.name).toBe('button')
  expect(textOfId(html, control?.attributes['aria-labelledby'] as string)).toBe('Notifications')
})

test('Toggle without a visible label falls back to aria-label', () => {
  const html = renderToStaticMarkup(<Toggle aria-label="Mute chat" />)
  const control = byRole(html, 'switch')[0]
  expect(control?.attributes['aria-label']).toBe('Mute chat')
  expect(control?.attributes['aria-checked']).toBe('false')
})

test("Toggle's description is referenced rather than merged into the name", () => {
  const html = renderToStaticMarkup(<Toggle label="Auto-download" description="On Wi-Fi only" />)
  const control = byRole(html, 'switch')[0]
  expect(textOfId(html, control?.attributes['aria-describedby'] as string)).toBe('On Wi-Fi only')
})

// ── Checkbox ────────────────────────────────────────────────────────────────────

test('Checkbox is a real input, so Space, form participation and mixed state come from the platform', () => {
  const html = renderToStaticMarkup(<Checkbox label="Pin to top" name="pin" value="1" defaultChecked />)
  const input = find(html, { name: 'input', attributes: { type: 'checkbox' } })[0]
  expect(input?.attributes['name']).toBe('pin')
  expect(input?.attributes['checked']).toBeDefined()
  expect(find(html, { name: 'label' })[0]?.attributes['for']).toBe(input?.attributes['id'] as string)
})

// ── RadioGroup ──────────────────────────────────────────────────────────────────

test('RadioGroup is a named group whose inputs share a name, with exactly one selected', () => {
  const html = renderToStaticMarkup(
    <RadioGroup
      name="privacy"
      label="Who can add me"
      value="contacts"
      options={[
        { value: 'everybody', label: 'Everybody' },
        { value: 'contacts', label: 'My contacts' },
        { value: 'nobody', label: 'Nobody', disabled: true },
      ]}
    />,
  )
  const group = byRole(html, 'radiogroup')[0]
  expect(textOfId(html, group?.attributes['aria-labelledby'] as string)).toBe('Who can add me')

  const radios = find(html, { name: 'input', attributes: { type: 'radio' } })
  expect(radios).toHaveLength(3)
  expect(radios.every((radio) => radio.attributes['name'] === 'privacy')).toBe(true)
  expect(radios.filter((radio) => Object.hasOwn(radio.attributes, 'checked'))).toHaveLength(1)
  expect(radios.filter((radio) => Object.hasOwn(radio.attributes, 'disabled'))).toHaveLength(1)
})
