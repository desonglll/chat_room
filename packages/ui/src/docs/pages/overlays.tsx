import { useRef, useState } from 'react'
import { Button } from '../../primitives/Button'
import { ContextMenu } from '../../primitives/ContextMenu'
import { IconButton } from '../../primitives/IconButton'
import { Menu } from '../../primitives/Menu'
import { MenuList, type MenuItem } from '../../primitives/MenuList'
import { Popover } from '../../primitives/Popover'
import { TextField } from '../../primitives/TextField'
import { Tooltip } from '../../primitives/Tooltip'
import { DemoIcon, Filler, Row, Section, type DocPage } from '../kit'

const PLACEMENTS = ['top', 'bottom', 'left', 'right'] as const

const MENU_ITEMS: MenuItem[] = [
  { id: 'reply', label: 'Reply', textValue: 'Reply', icon: <DemoIcon />, hint: 'Ctrl R' },
  { id: 'forward', label: 'Forward', textValue: 'Forward', icon: <DemoIcon shape="circle" /> },
  { id: 'copy', label: 'Copy text', textValue: 'Copy', disabled: true },
  { id: 'pin', label: 'Pin', textValue: 'Pin', selected: true },
  { id: 'delete', label: 'Delete', textValue: 'Delete', danger: true, separatorBefore: true, icon: <DemoIcon /> },
]

function PopoverDemo() {
  const [open, setOpen] = useState<string | null>(null)
  // One stable ref object per placement. A `{ current: node }` literal built during render would
  // change identity on every render and defeat the position hook's memoisation.
  const top = useRef<HTMLButtonElement>(null)
  const right = useRef<HTMLButtonElement>(null)
  const bottom = useRef<HTMLButtonElement>(null)
  const left = useRef<HTMLButtonElement>(null)
  const anchors = { top, right, bottom, left }

  return (
    <Section title="Placements (each flips when it would leave the viewport)">
      <Row>
        {PLACEMENTS.map((placement) => (
          <Button
            key={placement}
            variant="tonal"
            ref={anchors[placement]}
            aria-expanded={open === placement}
            onClick={() => setOpen(open === placement ? null : placement)}
          >
            {placement}
          </Button>
        ))}
      </Row>
      {PLACEMENTS.map((placement) => (
        <Popover
          key={placement}
          open={open === placement}
          onClose={() => setOpen(null)}
          anchor={anchors[placement]}
          insideRefs={[anchors[placement]]}
          placement={placement}
          role="dialog"
          aria-label={`${placement} panel`}
          className="doc-popover"
        >
          <Filler lines={1} />
          <Row>
            <TextField label="Focusable" size="sm" />
            <Button size="sm" onClick={() => setOpen(null)}>
              Close
            </Button>
          </Row>
        </Popover>
      ))}
    </Section>
  )
}

function MenuDemo() {
  const trigger = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const [chosen, setChosen] = useState('nothing yet')

  return (
    <>
      <Section title="Anchored menu">
        <Row>
          <Button
            ref={trigger}
            variant="tonal"
            aria-haspopup="menu"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            Open menu
          </Button>
          <span className="doc-note">last selected: {chosen}</span>
        </Row>
        <Menu
          open={open}
          onClose={() => setOpen(false)}
          anchor={trigger}
          triggerRef={trigger}
          aria-label="Message actions"
          items={MENU_ITEMS.map((item) => ({ ...item, onSelect: () => setChosen(item.id) }))}
        />
      </Section>
      <Section title="Row states, rendered inline so every state is visible at once">
        <div className="doc-menu-plate">
          <MenuList items={MENU_ITEMS} aria-label="Row states" autoFocus={false} />
        </div>
      </Section>
    </>
  )
}

function ContextMenuDemo() {
  const [chosen, setChosen] = useState('nothing yet')
  return (
    <>
      <Section title="Right-click, long-press, or focus it and press Shift+F10">
        <ContextMenu
          focusable
          aria-label="Region actions"
          items={MENU_ITEMS.map((item) => ({ ...item, onSelect: () => setChosen(item.id) }))}
        >
          <div className="doc-context-target">
            <strong>A region with a context menu</strong>
            <Filler lines={1} />
          </div>
        </ContextMenu>
      </Section>
      <Section title="Result">
        <span className="doc-note">last selected: {chosen}</span>
      </Section>
    </>
  )
}

function TooltipDemo() {
  return (
    <>
      <Section title="Hover, or Tab to it - focus shows the tip with no delay">
        <Row>
          {PLACEMENTS.map((placement) => (
            <Tooltip key={placement} label={`Tooltip on ${placement}`} placement={placement}>
              <Button variant="tonal">{placement}</Button>
            </Tooltip>
          ))}
        </Row>
      </Section>
      <Section title="On an icon-only control, supplementing its name rather than replacing it">
        <Row>
          <Tooltip label="Attach a file">
            <IconButton label="Attach file">
              <DemoIcon shape="plus" />
            </IconButton>
          </Tooltip>
          <Tooltip label="Never shown" disabled>
            <IconButton label="Tooltip disabled">
              <DemoIcon />
            </IconButton>
          </Tooltip>
        </Row>
      </Section>
    </>
  )
}

export const overlayPages: DocPage[] = [
  {
    id: 'popover',
    title: 'Popover',
    summary: 'The anchored floating layer that Menu, ContextMenu and Tooltip are built on.',
    notes: [
      'Placement: 12 values, flipping to the opposite side when the requested one does not fit and then clamping inside the viewport. The geometry is pure and unit tested (internal/positioning.ts).',
      'Focus: three modes. "trap" (the default) confines Tab and restores focus on close; "move" only puts focus on the first control inside, which is what Menu needs; "none" never touches focus, which is what Tooltip needs - taking focus would blur its own trigger and close it.',
      'A focusin anywhere outside a trapped panel pulls focus back, so focus cannot escape by any route, not just by Tab.',
      'Escape closes the TOPMOST layer only, so a menu opened inside a popover takes the first Escape and the popover takes the second.',
      'Dismissal uses pointerdown, not click: by the time a click completes the user may already have pressed something on the layer underneath.',
      'Motion: scales from the corner nearest the anchor on --tg-ease-overshoot over --tg-duration-panel, and exits on --tg-ease-accelerate over --tg-duration-exit. It stays mounted for the exit (internal/usePresence.ts).',
    ],
    Demo: PopoverDemo,
  },
  {
    id: 'menu',
    title: 'Menu',
    summary: 'Anchored menu of uniform rows, with the full ARIA menu keyboard contract.',
    notes: [
      'ArrowUp / ArrowDown move and wrap, skipping disabled rows. Home / End go to the first / last enabled row.',
      'Typing a printable character jumps to the next row starting with it, and repeating the character cycles through the matches.',
      'Tab and Shift+Tab CLOSE the menu rather than cycling inside it. That is the ARIA menu pattern and what Telegram Desktop does; it is why the popover runs in focus="move" mode here instead of trapping.',
      'Exactly one row is in the tab order at a time, and it is computed during render - not set by an effect - so a menu that has not been auto-focused is still reachable.',
      'A selected row becomes role="menuitemradio" with aria-checked, so the check is not conveyed by the glyph alone.',
    ],
    Demo: MenuDemo,
  },
  {
    id: 'context-menu',
    title: 'ContextMenu',
    summary: 'Right-click, long-press and keyboard context menu around any region.',
    notes: [
      'Three ways in: contextmenu (right click) at the pointer, a 500ms touch hold at the touch point, and Shift+F10 or the ContextMenu key anchored to the region. The keyboard route is the one usually missing.',
      'A hold is cancelled once the finger moves more than longPressSlop pixels, so it does not fight a scroll gesture.',
      'The wrapper adds no tab stop by default: it normally wraps something already focusable. focusable opts in for a region of plain content.',
    ],
    Demo: ContextMenuDemo,
  },
  {
    id: 'tooltip',
    title: 'Tooltip',
    summary: 'Hover and focus tooltip, wired with aria-describedby.',
    notes: [
      'It opens on FOCUS with no delay as well as on hover after openDelay, because a pointer-only tooltip is invisible to a keyboard user.',
      'Escape dismisses it while the trigger keeps focus, and the handler neither stops nor prevents the event, so a dialog behind it still sees its own Escape.',
      'It never takes focus and never registers as a dismissal layer. pointer-events are off, so it cannot swallow a click meant for the trigger.',
      'Touch is deliberately ignored: on touch there is no hover, and a long press belongs to ContextMenu.',
    ],
    Demo: TooltipDemo,
  },
]
