import { useState } from 'react'
import { Button } from '../../primitives/Button'
import { IconButton } from '../../primitives/IconButton'
import { Ripple } from '../../primitives/Ripple'
import { DemoIcon, Row, Section, State, type DocPage } from '../kit'

const VARIANTS = ['filled', 'tonal', 'text', 'danger'] as const
const SIZES = ['sm', 'md', 'lg'] as const

function ButtonDemo() {
  return (
    <>
      <Section title="Variants">
        <Row>
          {VARIANTS.map((variant) => (
            <Button key={variant} variant={variant}>
              {variant}
            </Button>
          ))}
        </Row>
      </Section>
      <Section title="Sizes">
        <Row>
          {SIZES.map((size) => (
            <Button key={size} size={size}>
              size {size}
            </Button>
          ))}
        </Row>
      </Section>
      <Section title="Shapes and width">
        <Row>
          <Button shape="pill">pill</Button>
          <Button shape="control">control</Button>
        </Row>
        <Button fullWidth>full width</Button>
      </Section>
      <Section title="With icons">
        <Row>
          <Button startIcon={<DemoIcon shape="plus" />}>start icon</Button>
          <Button variant="tonal" endIcon={<DemoIcon shape="circle" />}>
            end icon
          </Button>
        </Row>
      </Section>
      <Section title="States">
        <Row>
          <Button loading>loading</Button>
          <Button disabled>disabled</Button>
          <Button variant="tonal" disabled>
            disabled tonal
          </Button>
          <Button ripple={false}>no ripple</Button>
        </Row>
      </Section>
    </>
  )
}

function IconButtonDemo() {
  const [muted, setMuted] = useState(false)
  return (
    <>
      <Section title="Variants">
        <Row>
          {(['plain', 'filled', 'tonal', 'danger'] as const).map((variant) => (
            <IconButton key={variant} label={variant} variant={variant}>
              <DemoIcon />
            </IconButton>
          ))}
        </Row>
      </Section>
      <Section title="Sizes and shapes">
        <Row>
          {SIZES.map((size) => (
            <IconButton key={size} label={`size ${size}`} size={size}>
              <DemoIcon />
            </IconButton>
          ))}
          <IconButton label="rounded" shape="rounded">
            <DemoIcon />
          </IconButton>
        </Row>
      </Section>
      <Section title="Toggle state (aria-pressed) and disabled">
        <Row>
          <IconButton label="Mute chat" selected={muted} onClick={() => setMuted((v) => !v)}>
            <DemoIcon shape="circle" />
          </IconButton>
          <IconButton label="Unavailable" disabled>
            <DemoIcon />
          </IconButton>
        </Row>
      </Section>
    </>
  )
}

function RippleDemo() {
  return (
    <>
      <Section title="Press anywhere: the wave starts where you clicked">
        <div className="doc-ripple-surface">
          <Ripple />
          <span>click near a corner, then near the centre</span>
        </div>
      </Section>
      <Section title="Tints">
        <Row>
          <div className="doc-ripple-surface doc-ripple-surface--accent">
            <Ripple tint="inverse" />
            <span>inverse, on an accent fill</span>
          </div>
          <div className="doc-ripple-surface">
            <Ripple tint="accent" />
            <span>accent</span>
          </div>
        </Row>
      </Section>
      <Section title="Compact (Telegram's handheld variant, starts at 27%)">
        <div className="doc-ripple-surface doc-ripple-surface--short">
          <Ripple compact />
          <span>compact</span>
        </div>
      </Section>
      <Section title="Keyboard activation has no pointer, so the wave starts at the centre">
        <State label="Tab to it, then press Enter or Space">
          <Button variant="tonal">keyboard ripple</Button>
        </State>
      </Section>
    </>
  )
}

export const buttonPages: DocPage[] = [
  {
    id: 'button',
    title: 'Button',
    summary: 'Text button. Four Telegram treatments, three sizes, two shapes.',
    notes: [
      'Keyboard: a real <button>, so Enter and Space activate and the disabled state is the platform one.',
      'type defaults to "button": a primitive that submits the nearest form by accident is a bug that surfaces late.',
      'Motion: background and colour on --tg-transition-press (100ms); the ripple on --tg-ripple-duration (700ms).',
      'Contrast: filled and danger paint --tg-text-on-accent on the fill, 3.31:1 on day. Kept deliberately, per the contrast ruling on the TG-606 card.',
      'loading keeps the accessible name and sets aria-busy rather than swapping the label out of the tree.',
    ],
    Demo: ButtonDemo,
  },
  {
    id: 'icon-button',
    title: 'IconButton',
    summary: 'Square or circular control whose whole content is an icon.',
    notes: [
      'label is REQUIRED and becomes both aria-label and the native title. An icon-only button with no name is the most common accessibility defect in a chat client.',
      'selected sets aria-pressed, and the selected look changes icon colour as well as background, so the state is not colour-only.',
      'Hit area: md and lg are at least --tg-touch-min (44px). sm is 32px and belongs only inside a larger row target.',
    ],
    Demo: IconButtonDemo,
  },
  {
    id: 'ripple',
    title: 'Ripple',
    summary: "Telegram's press feedback. Expands from the pointer and does not migrate.",
    notes: [
      'Origin: the wave is centred on the pointer and STAYS there. Material translates the circle from the pointer toward the element centre while it grows.',
      'Size: diameter is max(width, height) of the host, not the diagonal, so it is not guaranteed to reach the far corner. That under-coverage is Telegram, not a bug.',
      'Timing: --tg-ripple-scale-start 0 to --tg-ripple-scale-end 2 over --tg-ripple-duration 700ms, roughly three times Material, and it always runs to completion.',
      'Keyboard: Enter and Space spawn a wave from the host centre, because there is no pointer.',
      'Reduced motion: TG-009 collapses both scales to 1 and shortens the duration to --tg-duration-fade, so the wave flashes without travelling.',
    ],
    Demo: RippleDemo,
  },
]
