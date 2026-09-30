import { useRef, useState } from 'react'
import { Button } from '../../primitives/Button'
import { Checkbox } from '../../primitives/Checkbox'
import { Modal } from '../../primitives/Modal'
import { Sheet } from '../../primitives/Sheet'
import { TextField } from '../../primitives/TextField'
import { Filler, Row, Section, Stack, type DocPage } from '../kit'

const SIDES = ['bottom', 'left', 'right', 'top'] as const

function ModalDemo() {
  const [open, setOpen] = useState<string | null>(null)
  const initial = useRef<HTMLInputElement>(null)

  return (
    <>
      <Section title="Sizes">
        <Row>
          {(['sm', 'md', 'lg'] as const).map((size) => (
            <Button key={size} variant="tonal" onClick={() => setOpen(size)}>
              {size}
            </Button>
          ))}
        </Row>
      </Section>
      <Section title="Variants">
        <Row>
          <Button variant="tonal" onClick={() => setOpen('focus')}>
            initial focus on the field
          </Button>
          <Button variant="tonal" onClick={() => setOpen('sticky')}>
            backdrop and Escape disabled
          </Button>
          <Button variant="tonal" onClick={() => setOpen('long')}>
            scrolling body
          </Button>
          <Button variant="tonal" onClick={() => setOpen('bare')}>
            no title, no close button
          </Button>
        </Row>
      </Section>

      {(['sm', 'md', 'lg'] as const).map((size) => (
        <Modal
          key={size}
          open={open === size}
          onClose={() => setOpen(null)}
          size={size}
          title={`Delete chat (${size})`}
          description="This cannot be undone."
          footer={
            <>
              <Button variant="text" onClick={() => setOpen(null)}>
                Cancel
              </Button>
              <Button variant="danger" onClick={() => setOpen(null)}>
                Delete
              </Button>
            </>
          }
        >
          <Checkbox label="Also delete for the other side" />
        </Modal>
      ))}

      <Modal
        open={open === 'focus'}
        onClose={() => setOpen(null)}
        title="Rename"
        initialFocusRef={initial}
        footer={<Button onClick={() => setOpen(null)}>Save</Button>}
      >
        <Stack>
          <Button variant="text">a button before the field</Button>
          <TextField ref={initial} label="New title" fullWidth />
        </Stack>
      </Modal>

      <Modal
        open={open === 'sticky'}
        onClose={() => setOpen(null)}
        title="Confirm"
        closeOnBackdrop={false}
        closeOnEscape={false}
        description="Only the buttons close this one."
        footer={<Button onClick={() => setOpen(null)}>Got it</Button>}
      />

      <Modal open={open === 'long'} onClose={() => setOpen(null)} title="Terms">
        <Stack>
          {Array.from({ length: 12 }, (_, index) => (
            <Filler key={index} lines={2} />
          ))}
        </Stack>
      </Modal>

      <Modal open={open === 'bare'} onClose={() => setOpen(null)} ariaLabel="Bare dialog" showClose={false}>
        <Stack>
          <Filler lines={1} />
          <Button onClick={() => setOpen(null)}>Close</Button>
        </Stack>
      </Modal>
    </>
  )
}

function SheetDemo() {
  const [open, setOpen] = useState<string | null>(null)
  return (
    <>
      <Section title="Sides">
        <Row>
          {SIDES.map((side) => (
            <Button key={side} variant="tonal" onClick={() => setOpen(side)}>
              {side}
            </Button>
          ))}
        </Row>
      </Section>
      {SIDES.map((side) => (
        <Sheet
          key={side}
          open={open === side}
          onClose={() => setOpen(null)}
          side={side}
          title={`${side} sheet`}
          description="Same dialog semantics as Modal, different geometry."
          footer={<Button onClick={() => setOpen(null)}>Done</Button>}
        >
          <Stack>
            <TextField label="Something focusable" fullWidth />
            <Checkbox label="And something else" />
            <Filler lines={2} />
          </Stack>
        </Sheet>
      ))}
    </>
  )
}

const DIALOG_NOTES = [
  'Focus: it moves into the dialog on open (initialFocusRef, else the first tab stop, else the dialog itself at tabIndex -1) and returns to whatever had it before, on close.',
  'Tab and Shift+Tab cycle inside and cannot reach the page behind. A focusin outside pulls focus back, which covers focus arriving by any route other than Tab.',
  'Escape closes the topmost layer only. Open a Menu inside one of these and the first Escape closes the menu.',
  'The document is scroll-locked while it is open, with scrollbar-width compensation so the page does not jump sideways. The lock is reference counted for nested dialogs.',
  'Either title (which becomes aria-labelledby) or ariaLabel is required in practice: an unnamed dialog announces as "dialog" and nothing else.',
]

export const dialogPages: DocPage[] = [
  {
    id: 'modal',
    title: 'Modal',
    summary: 'Centred modal dialog.',
    notes: [
      ...DIALOG_NOTES,
      'Motion: scale plus fade in on --tg-duration-enter with --tg-ease-overshoot; out on --tg-duration-exit with --tg-ease-accelerate. The backdrop only fades, on --tg-transition-fade, which TG-009 keeps alive under reduced motion.',
      'It stays mounted for the exit animation (internal/usePresence.ts), which is why closing does not flash.',
    ],
    Demo: ModalDemo,
  },
  {
    id: 'sheet',
    title: 'Sheet',
    summary: 'Edge-attached panel: bottom sheet on a handheld, sliding side panel on a desktop.',
    notes: [
      ...DIALOG_NOTES,
      'Motion: slides in from `side` on --tg-transition-slide (Android EASE_OUT_QUINT, TG-009 --tg-ease-decelerate). Under reduced motion the slide collapses to 1ms and the panel appears in place.',
      'Drag-to-dismiss is NOT here: it needs a gesture layer and a JS spring, and belongs to the feature that owns the sheet. The grabber affordance is rendered so the shape is right when that lands.',
    ],
    Demo: SheetDemo,
  },
]
