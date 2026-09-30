import { useState } from 'react'
import { Checkbox } from '../../primitives/Checkbox'
import { RadioGroup } from '../../primitives/Radio'
import { TextField } from '../../primitives/TextField'
import { Toggle } from '../../primitives/Toggle'
import { DemoIcon, Row, Section, Stack, State, type DocPage } from '../kit'

function TextFieldDemo() {
  const [value, setValue] = useState('Ada')
  return (
    <>
      <Section title="Label float: empty, filled, focused">
        <Stack>
          <TextField label="Chat title" fullWidth />
          <TextField label="Chat title" fullWidth value={value} onChange={(e) => setValue(e.target.value)} />
        </Stack>
      </Section>
      <Section title="Sizes">
        <Stack>
          <TextField label="small" size="sm" fullWidth />
          <TextField label="medium" size="md" fullWidth />
          <TextField label="large" size="lg" fullWidth />
        </Stack>
      </Section>
      <Section title="Hint, error, required, disabled">
        <Stack>
          <TextField label="Username" hint="Letters, digits and underscore" fullWidth />
          <TextField label="Username" error="This username is taken" fullWidth defaultValue="ada" />
          <TextField label="Username" required fullWidth />
          <TextField label="Username" disabled fullWidth defaultValue="locked" />
        </Stack>
      </Section>
      <Section title="Adornments and clear">
        <Stack>
          <TextField label="Search" startAdornment={<DemoIcon shape="circle" />} fullWidth />
          <TextField
            label="Search"
            clearable
            fullWidth
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onClear={() => setValue('')}
          />
        </Stack>
      </Section>
      <Section title="Multiline">
        <TextField label="About" multiline rows={3} fullWidth />
      </Section>
      <Section title="No label: a plain placeholder field">
        <TextField placeholder="Message" fullWidth />
      </Section>
    </>
  )
}

function ToggleDemo() {
  const [on, setOn] = useState(true)
  return (
    <>
      <Section title="Bare control">
        <Row>
          <Toggle aria-label="Unchecked" />
          <Toggle aria-label="Checked" defaultChecked />
          <Toggle aria-label="Small" size="sm" defaultChecked />
          <Toggle aria-label="Disabled" disabled />
          <Toggle aria-label="Disabled on" disabled defaultChecked />
        </Row>
      </Section>
      <Section title="Settings rows">
        <Stack>
          <Toggle label="Notifications" checked={on} onCheckedChange={setOn} />
          <Toggle label="Auto-download media" description="On Wi-Fi only" defaultChecked />
          <Toggle label="Label after the switch" labelPlacement="end" />
          <Toggle label="Unavailable here" description="Disabled row" disabled />
        </Stack>
      </Section>
    </>
  )
}

function CheckboxDemo() {
  const [partial, setPartial] = useState(true)
  return (
    <>
      <Section title="States">
        <Stack>
          <Checkbox label="Unchecked" />
          <Checkbox label="Checked" defaultChecked />
          <Checkbox label="Indeterminate" indeterminate={partial} onCheckedChange={() => setPartial(false)} />
          <Checkbox label="Disabled" disabled />
          <Checkbox label="Disabled, checked" disabled defaultChecked />
        </Stack>
      </Section>
      <Section title="Shapes and sizes">
        <Row>
          <Checkbox label="square" defaultChecked />
          <Checkbox label="round" shape="round" defaultChecked />
          <Checkbox label="small" size="sm" defaultChecked />
        </Row>
      </Section>
      <Section title="With a description">
        <Checkbox label="Delete for everyone" description="The message disappears for both sides" />
      </Section>
    </>
  )
}

function RadioDemo() {
  const [value, setValue] = useState('contacts')
  return (
    <>
      <Section title="Vertical group">
        <RadioGroup
          name="doc-privacy"
          label="Who can add me to groups"
          value={value}
          onValueChange={setValue}
          options={[
            { value: 'everybody', label: 'Everybody' },
            { value: 'contacts', label: 'My contacts', description: 'Recommended' },
            { value: 'nobody', label: 'Nobody', disabled: true },
          ]}
        />
      </Section>
      <Section title="Horizontal group">
        <RadioGroup
          name="doc-sort"
          orientation="horizontal"
          aria-label="Sort order"
          defaultValue="recent"
          options={[
            { value: 'recent', label: 'Recent' },
            { value: 'name', label: 'Name' },
            { value: 'unread', label: 'Unread' },
          ]}
        />
      </Section>
      <Section title="Whole group disabled">
        <State label="disabled">
          <RadioGroup
            name="doc-locked"
            aria-label="Locked"
            disabled
            defaultValue="a"
            options={[
              { value: 'a', label: 'A' },
              { value: 'b', label: 'B' },
            ]}
          />
        </State>
      </Section>
    </>
  )
}

export const inputPages: DocPage[] = [
  {
    id: 'text-field',
    title: 'TextField',
    summary: 'Rounded bordered input with a label that floats into the border.',
    notes: [
      'The float is pure CSS (:focus-within and :placeholder-shown). Because the selector needs a placeholder to test emptiness, a label forces placeholder=" " and a caller-supplied placeholder is ignored.',
      'error sets aria-invalid, replaces the hint, and is announced (role="alert"). A field that only turns red says nothing to a screen reader or to a colour-blind user.',
      'Focus is a 2px accent border rather than an outline, because the floating label overlaps the border and an outline would be drawn through it. The border thickening is the non-colour signal.',
      'Motion: border and label on --tg-transition-panel (200ms, --tg-ease-decelerate).',
    ],
    Demo: TextFieldDemo,
  },
  {
    id: 'toggle',
    title: 'Toggle',
    summary: "Telegram's settings switch.",
    notes: [
      'role="switch", so a screen reader says on/off rather than checked/unchecked.',
      'Keyboard: a real <button>, so Space and Enter both flip it. Clicking the label flips it too, via aria-labelledby plus htmlFor.',
      "Motion: the thumb slides on --tg-transition-spring-smooth. That is TG-009's critically damped spring (zeta = 1.0000), so it arrives without overshoot.",
    ],
    Demo: ToggleDemo,
  },
  {
    id: 'checkbox',
    title: 'Checkbox',
    summary: 'Square or round checkbox, with an indeterminate state.',
    notes: [
      'A real <input type="checkbox">, visually hidden rather than replaced. That is where Space activation, form participation, the mixed state and label association come from.',
      'indeterminate is a DOM property with no React prop, so it is set in an effect. The element then reports aria-checked="mixed" natively.',
      'Motion: the tick scales in on --tg-transition-spring-smooth; the box fill changes on --tg-transition-press.',
    ],
    Demo: CheckboxDemo,
  },
  {
    id: 'radio',
    title: 'Radio / RadioGroup',
    summary: 'Single choice from a small set.',
    notes: [
      'Arrow-key navigation and the single group tab stop come from the browser, because these are real radios sharing a name. Reimplementing that on role="radio" divs is how a custom radio ends up worse than the platform.',
      'RadioGroup adds only what the platform lacks: role="radiogroup" with a name, and Home / End, which browsers do not implement for radios.',
      'Motion: the dot scales in on --tg-transition-spring-smooth.',
    ],
    Demo: RadioDemo,
  },
]
