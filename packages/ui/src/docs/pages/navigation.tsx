import { useState } from 'react'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { ScrollArea } from '../../primitives/ScrollArea'
import { Tabs } from '../../primitives/Tabs'
import { DemoIcon, Filler, Row, Section, Stack, State, type DocPage } from '../kit'

const FOLDERS = [
  { id: 'all', label: 'All' },
  { id: 'unread', label: 'Unread', badge: <Badge count={12} size="sm" /> },
  { id: 'groups', label: 'Groups', icon: <DemoIcon /> },
  { id: 'channels', label: 'Channels', disabled: true },
]

function TabsDemo() {
  const [manual, setManual] = useState('all')
  return (
    <>
      <Section title="Underline (automatic activation: selection follows focus)">
        <Tabs
          items={FOLDERS}
          aria-label="Folders, underline"
          panels={{
            all: <Filler lines={1} />,
            unread: <Filler lines={2} />,
            groups: <Filler lines={3} />,
          }}
        />
      </Section>
      <Section title="Segmented">
        <Tabs items={FOLDERS} variant="segmented" aria-label="Folders, segmented" />
      </Section>
      <Section title="Stretched to the full width">
        <Tabs items={FOLDERS.slice(0, 3)} stretch aria-label="Folders, stretched" />
      </Section>
      <Section title="Manual activation: arrow to move focus, Enter or Space to select">
        <Stack>
          <Tabs items={FOLDERS} activation="manual" value={manual} onValueChange={setManual} aria-label="Manual" />
          <span className="doc-note">selected: {manual}</span>
        </Stack>
      </Section>
      <Section title="Vertical orientation (ArrowUp / ArrowDown)">
        <div className="doc-vertical-tabs">
          <Tabs items={FOLDERS} orientation="vertical" aria-label="Folders, vertical" />
        </div>
      </Section>
    </>
  )
}

function ScrollAreaDemo() {
  const [items, setItems] = useState(20)
  return (
    <>
      <Section title="Vertical, overlay bar (hover or scroll to reveal it)">
        <ScrollArea className="doc-scroll">
          <Stack>
            {Array.from({ length: items }, (_, index) => (
              <Button key={index} variant="text" fullWidth>
                row {index + 1}
              </Button>
            ))}
          </Stack>
        </ScrollArea>
        <Row>
          <Button size="sm" variant="tonal" onClick={() => setItems((n) => n + 10)}>
            add rows
          </Button>
        </Row>
      </Section>
      <Section title="Always-visible bar">
        <ScrollArea className="doc-scroll" overlay={false}>
          <Stack>
            {Array.from({ length: 12 }, (_, index) => (
              <Filler key={index} lines={1} />
            ))}
          </Stack>
        </ScrollArea>
      </Section>
      <Section title="Horizontal">
        <ScrollArea className="doc-scroll doc-scroll--horizontal" orientation="horizontal">
          <div className="doc-scroll__track">
            {Array.from({ length: 24 }, (_, index) => (
              <span key={index} className="doc-chip">
                chip {index + 1}
              </span>
            ))}
          </div>
        </ScrollArea>
      </Section>
      <Section title="Faded edges, and a keyboard-focusable region of plain text">
        <Stack>
          <ScrollArea className="doc-scroll" fadeEdges>
            <Stack>
              {Array.from({ length: 10 }, (_, index) => (
                <Filler key={index} lines={1} />
              ))}
            </Stack>
          </ScrollArea>
          <State label="focusable">
            <ScrollArea className="doc-scroll" focusable aria-label="Transcript">
              <Stack>
                {Array.from({ length: 10 }, (_, index) => (
                  <Filler key={index} lines={1} />
                ))}
              </Stack>
            </ScrollArea>
          </State>
        </Stack>
      </Section>
    </>
  )
}

export const navigationPages: DocPage[] = [
  {
    id: 'tabs',
    title: 'Tabs',
    summary: 'Tab strip with the full ARIA tabs keyboard contract.',
    notes: [
      'The whole strip is ONE tab stop. Arrow keys move between tabs inside it, wrapping and skipping disabled ones; Home and End jump to the ends. Leaving every tab tabbable is what turns a fifteen-folder filter bar into fifteen tab stops.',
      'activation="automatic" (the default) selects on focus. Use "manual" when a tab loads something expensive, so arrowing past it does not fetch it.',
      'Passing `panels` wires aria-controls and aria-labelledby both ways and renders the active tabpanel with tabIndex 0. Omitting it omits aria-controls rather than pointing at an element that does not exist.',
      'The underline is a pseudo-element on the selected tab, so it cross-fades and scales instead of travelling. A single sliding bar would need JS measurement that is wrong on the first frame and after every font load. This is the one deliberate visual deviation from tweb in this component.',
    ],
    Demo: TabsDemo,
  },
  {
    id: 'scroll-area',
    title: 'ScrollArea',
    summary: "Native scrolling with Telegram's thin overlay scrollbar.",
    notes: [
      'It scrolls natively, which is the point: a JS-driven scrollbar breaks momentum scrolling, scrollIntoView and find-in-page, and would fight react-virtuoso in TG-101.',
      'Two styling mechanisms on purpose: scrollbar-color / scrollbar-width is the standard and what Firefox honours; ::-webkit-scrollbar is what Chromium and Safari honour and the only way to make the bar float.',
      'focusable is OFF by default. A scroll region should be keyboard focusable when it has no focusable content, but a chat list is full of focusable children and a stop around each one doubles the tab stops in the window. Set it for a region of plain text.',
      'Motion: the overlay bar fades on --tg-transition-fade, which TG-009 keeps alive under reduced motion, so it still fades rather than snapping.',
    ],
    Demo: ScrollAreaDemo,
  },
]
