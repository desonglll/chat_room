import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { DialogSurface } from '../internal/DialogSurface'
import { Avatar } from '../primitives/Avatar'
import { Badge } from '../primitives/Badge'
import { MenuList, type MenuItem } from '../primitives/MenuList'
import { Popover } from '../primitives/Popover'
import { ScrollArea } from '../primitives/ScrollArea'
import { Skeleton } from '../primitives/Skeleton'
import { Spinner } from '../primitives/Spinner'
import { Tabs } from '../primitives/Tabs'
import { byRole, find, openTags, textOfId } from './markup'

const items: MenuItem[] = [
  { id: 'reply', label: 'Reply', textValue: 'Reply' },
  { id: 'copy', label: 'Copy', textValue: 'Copy', disabled: true },
  { id: 'pin', label: 'Pin', textValue: 'Pin', selected: true },
  { id: 'delete', label: 'Delete', textValue: 'Delete', danger: true, separatorBefore: true },
]

// ── MenuList ────────────────────────────────────────────────────────────────────

test('MenuList is a named menu of menuitems', () => {
  const html = renderToStaticMarkup(<MenuList items={items} aria-label="Message actions" autoFocus={false} />)
  expect(byRole(html, 'menu')[0]?.attributes['aria-label']).toBe('Message actions')
  expect(byRole(html, 'menuitem')).toHaveLength(3)
  expect(byRole(html, 'menuitemradio')).toHaveLength(1)
  expect(byRole(html, 'menuitemradio')[0]?.attributes['aria-checked']).toBe('true')
  expect(byRole(html, 'separator')).toHaveLength(1)
})

test('MenuList has exactly ONE tab stop, on the first enabled row', () => {
  const html = renderToStaticMarkup(<MenuList items={items} aria-label="Message actions" autoFocus={false} />)
  const rows = [...byRole(html, 'menuitem'), ...byRole(html, 'menuitemradio')]
  expect(rows.filter((row) => row.attributes['tabindex'] === '0')).toHaveLength(1)
  expect(rows.filter((row) => row.attributes['tabindex'] === '-1')).toHaveLength(3)
})

test('MenuList puts its single tab stop past a leading disabled row', () => {
  const leadingDisabled: MenuItem[] = [
    { id: 'a', label: 'A', disabled: true },
    { id: 'b', label: 'B' },
  ]
  const html = renderToStaticMarkup(<MenuList items={leadingDisabled} aria-label="x" autoFocus={false} />)
  const rows = byRole(html, 'menuitem')
  expect(rows[0]?.attributes['tabindex']).toBe('-1')
  expect(rows[1]?.attributes['tabindex']).toBe('0')
})

test('a disabled row is aria-disabled rather than removed, so the menu shape stays stable', () => {
  const html = renderToStaticMarkup(<MenuList items={items} aria-label="x" autoFocus={false} />)
  expect(find(html, { attributes: { 'aria-disabled': 'true' } })).toHaveLength(1)
  expect(html).toContain('Copy')
})

// ── Popover ─────────────────────────────────────────────────────────────────────

test('a closed Popover renders nothing at all', () => {
  expect(
    renderToStaticMarkup(
      <Popover open={false} anchor={null}>
        panel
      </Popover>,
    ),
  ).toBe('')
})

// ── Modal / Sheet, through the surface they share ────────────────────────────────

test('a dialog is modal, named by its title, and described by its description', () => {
  const html = renderToStaticMarkup(
    <DialogSurface inline open kind="modal" onClose={() => {}} title="Delete chat" description="This cannot be undone.">
      body
    </DialogSurface>,
  )
  const dialog = byRole(html, 'dialog')[0]
  expect(dialog?.attributes['aria-modal']).toBe('true')
  expect(textOfId(html, dialog?.attributes['aria-labelledby'] as string)).toBe('Delete chat')
  expect(textOfId(html, dialog?.attributes['aria-describedby'] as string)).toBe('This cannot be undone.')
  // tabIndex -1 so focus can land on the surface itself when it contains no tab stop.
  expect(dialog?.attributes['tabindex']).toBe('-1')
})

test('a dialog with no title falls back to an explicit name rather than being anonymous', () => {
  const html = renderToStaticMarkup(
    <DialogSurface inline open kind="modal" onClose={() => {}} showClose={false} ariaLabel="Emoji picker">
      body
    </DialogSurface>,
  )
  const dialog = byRole(html, 'dialog')[0]
  expect(dialog?.attributes['aria-label']).toBe('Emoji picker')
  expect(dialog?.attributes['aria-labelledby']).toBeUndefined()
})

test("the dialog's close button is named, and the title is a heading", () => {
  const html = renderToStaticMarkup(
    <DialogSurface inline open kind="modal" onClose={() => {}} title="Settings" closeLabel="Close settings" />,
  )
  expect(find(html, { attributes: { class: 'tg-dialog__close' } })[0]?.attributes['aria-label']).toBe('Close settings')
  expect(find(html, { name: 'h2' })).toHaveLength(1)
})

test('a closed dialog renders nothing, and a sheet carries its side and a decorative grabber', () => {
  expect(renderToStaticMarkup(<DialogSurface inline open={false} kind="modal" onClose={() => {}} title="x" />)).toBe('')

  const sheet = renderToStaticMarkup(
    <DialogSurface inline open kind="sheet" side="bottom" grabber onClose={() => {}} title="Share" />,
  )
  expect(sheet).toContain('tg-dialog--bottom')
  expect(find(sheet, { attributes: { class: 'tg-dialog__grabber', 'aria-hidden': 'true' } })).toHaveLength(1)
})

// ── Tabs ────────────────────────────────────────────────────────────────────────

const tabs = [
  { id: 'all', label: 'All' },
  { id: 'unread', label: 'Unread' },
  { id: 'groups', label: 'Groups', disabled: true },
]

test('Tabs is a named tablist with exactly one selected tab and one tab stop', () => {
  const html = renderToStaticMarkup(<Tabs items={tabs} value="unread" aria-label="Chat folders" />)
  expect(byRole(html, 'tablist')[0]?.attributes['aria-label']).toBe('Chat folders')
  const all = byRole(html, 'tab')
  expect(all).toHaveLength(3)
  expect(all.filter((tab) => tab.attributes['aria-selected'] === 'true')).toHaveLength(1)
  expect(all.filter((tab) => tab.attributes['tabindex'] === '0')).toHaveLength(1)
  expect(all.find((tab) => tab.attributes['aria-selected'] === 'true')?.attributes['tabindex']).toBe('0')
})

test('Tabs wires aria-controls and aria-labelledby both ways when it owns the panels', () => {
  const html = renderToStaticMarkup(
    <Tabs items={tabs} value="all" aria-label="Chat folders" panels={{ all: 'everything', unread: 'unread only' }} />,
  )
  const panel = byRole(html, 'tabpanel')[0]
  const selected = byRole(html, 'tab').find((tab) => tab.attributes['aria-selected'] === 'true')
  expect(byRole(html, 'tabpanel')).toHaveLength(1)
  expect(selected?.attributes['aria-controls']).toBe(panel?.attributes['id'] as string)
  expect(panel?.attributes['aria-labelledby']).toBe(selected?.attributes['id'] as string)
  expect(html).toContain('everything')
  expect(html).not.toContain('unread only')
})

test('Tabs omits aria-controls when the consumer renders the panels, rather than dangling it', () => {
  const html = renderToStaticMarkup(<Tabs items={tabs} value="all" aria-label="Chat folders" />)
  expect(byRole(html, 'tab').every((tab) => tab.attributes['aria-controls'] === undefined)).toBe(true)
  expect(byRole(html, 'tabpanel')).toHaveLength(0)
})

test('Tabs selects the first ENABLED item when no value is given', () => {
  const html = renderToStaticMarkup(
    <Tabs
      items={[
        { id: 'x', label: 'X', disabled: true },
        { id: 'y', label: 'Y' },
      ]}
      aria-label="t"
    />,
  )
  const selected = byRole(html, 'tab').find((tab) => tab.attributes['aria-selected'] === 'true')
  expect(selected?.attributes['id']).toContain('tab-y')
})

// ── Avatar, Badge, Spinner, Skeleton, ScrollArea ─────────────────────────────────

test('Avatar with an image uses alt; without one it is a named role=img carrying initials', () => {
  const withImage = renderToStaticMarkup(<Avatar src="/a.png" label="Ada Lovelace" />)
  expect(find(withImage, { name: 'img' })[0]?.attributes['alt']).toBe('Ada Lovelace')

  const fallback = renderToStaticMarkup(<Avatar label="Ada Lovelace" />)
  const initials = byRole(fallback, 'img')[0]
  expect(initials?.attributes['aria-label']).toBe('Ada Lovelace')
  expect(fallback).toContain('AL')
})

test('Avatar palette slots are deterministic and the online dot has a non-colour signal', () => {
  const a = renderToStaticMarkup(<Avatar label="Ada Lovelace" />)
  const b = renderToStaticMarkup(<Avatar label="Ada Lovelace" />)
  expect(a).toBe(b)
  expect(renderToStaticMarkup(<Avatar label="Ada" colorIndex={3} />)).toContain('tg-avatar--slot-3')

  const online = renderToStaticMarkup(<Avatar label="Ada" online />)
  expect(online).toContain('data-tg-online')
  expect(find(online, { attributes: { class: 'tg-avatar__online', 'aria-hidden': 'true' } })).toHaveLength(1)
})

test('Badge truncates what it paints but never what it announces', () => {
  const html = renderToStaticMarkup(<Badge count={1284} />)
  expect(html).toContain('99+')
  expect(openTags(html)[0]?.attributes['aria-label']).toBe('1284')
  expect(openTags(html)[0]?.attributes['role']).toBe('status')
})

test('a Badge dot needs a label, and an unlabelled badge is hidden rather than announced empty', () => {
  const dot = renderToStaticMarkup(<Badge dot label="Unread" />)
  expect(openTags(dot)[0]?.attributes['aria-label']).toBe('Unread')
  expect(renderToStaticMarkup(<Badge dot />)).toContain('aria-hidden="true"')
})

test('Spinner is decorative unless named, and a named determinate one is a progressbar', () => {
  expect(openTags(renderToStaticMarkup(<Spinner />))[0]?.attributes['aria-hidden']).toBe('true')

  const busy = openTags(renderToStaticMarkup(<Spinner label="Loading messages" />))[0]
  expect(busy?.attributes['role']).toBe('status')

  const progress = openTags(renderToStaticMarkup(<Spinner label="Uploading" progress={0.42} />))[0]
  expect(progress?.attributes['role']).toBe('progressbar')
  expect(progress?.attributes['aria-valuenow']).toBe('42')
  expect(progress?.attributes['aria-valuemax']).toBe('100')
})

test('Skeleton is always hidden from assistive technology', () => {
  expect(openTags(renderToStaticMarkup(<Skeleton />))[0]?.attributes['aria-hidden']).toBe('true')
  const stack = renderToStaticMarkup(<Skeleton lines={3} />)
  expect(find(stack, { attributes: { class: 'tg-skeleton-stack' } })[0]?.attributes['aria-hidden']).toBe('true')
  expect(stack).toContain('tg-skeleton--short')
})

test('ScrollArea adds no tab stop by default and becomes a named group when asked', () => {
  const plain = openTags(renderToStaticMarkup(<ScrollArea>rows</ScrollArea>))[0]
  expect(plain?.attributes['tabindex']).toBeUndefined()

  const focusable = openTags(
    renderToStaticMarkup(
      <ScrollArea focusable aria-label="Transcript">
        t
      </ScrollArea>,
    ),
  )[0]
  expect(focusable?.attributes['tabindex']).toBe('0')
  expect(focusable?.attributes['role']).toBe('group')
  expect(focusable?.attributes['aria-label']).toBe('Transcript')
})
