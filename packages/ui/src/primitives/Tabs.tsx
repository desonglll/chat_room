import { useEffect, useId, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from 'react'
import { cx } from '../internal/cx'
import { nextFocusIndex } from '../internal/rovingFocus'
import { useControllable } from '../internal/useControllable'

export interface TabItem {
  id: string
  label: ReactNode
  icon?: ReactNode
  /** Trailing slot, typically a `Badge`. */
  badge?: ReactNode
  disabled?: boolean | undefined
}

/**
 * Tab strip with the full ARIA tabs keyboard contract.
 *
 * Roving tab stop: the strip is ONE tab stop, and Arrow keys move between tabs inside it. That is
 * the part most tab implementations get wrong by leaving every tab tabbable, which turns a
 * fifteen-folder chat filter bar into fifteen tab stops.
 *
 *   ArrowLeft / ArrowRight   previous / next enabled tab, wrapping (Up/Down when vertical)
 *   Home / End               first / last enabled tab
 *   Enter / Space            select, when `activation="manual"`
 *
 * `activation` defaults to `automatic` (selection follows focus), which is right when switching is
 * cheap. Use `manual` when a tab loads something expensive, so arrowing past it does not fetch it.
 *
 * Passing `panels` makes the component render the active `role="tabpanel"` with `aria-labelledby`
 * and `aria-controls` wired both ways. Omit it to render the panels yourself; `aria-controls` is
 * then omitted rather than pointing at an element that does not exist.
 */
export interface TabsProps {
  items: readonly TabItem[]
  value?: string | undefined
  defaultValue?: string | undefined
  onValueChange?: ((id: string) => void) | undefined
  /** `underline` is Telegram's sliding indicator; `segmented` is its pill group. */
  variant?: 'underline' | 'segmented' | undefined
  activation?: 'automatic' | 'manual' | undefined
  orientation?: 'horizontal' | 'vertical' | undefined
  /** Divide the available width equally between tabs, as Telegram's media panel does. */
  stretch?: boolean | undefined
  /** Accessible name of the tab strip. */
  'aria-label'?: string | undefined
  panels?: Readonly<Record<string, ReactNode>> | undefined
  className?: string | undefined
}

export function Tabs({
  items,
  value,
  defaultValue,
  onValueChange,
  variant = 'underline',
  activation = 'automatic',
  orientation = 'horizontal',
  stretch = false,
  'aria-label': ariaLabel,
  panels,
  className,
}: TabsProps) {
  const generated = useId()
  const tabs = useRef<(HTMLButtonElement | null)[]>([])
  const firstEnabled = items.find((item) => item.disabled !== true)?.id ?? items[0]?.id ?? ''
  const [selected, setSelected] = useControllable(value, defaultValue ?? firstEnabled, onValueChange)
  const selectedIndex = items.findIndex((item) => item.id === selected)
  const [focusIndex, setFocusIndex] = useState(-1)

  // Focus follows the roving index, but only once the user has entered the strip with a key: an
  // effect that focuses on mount would steal focus from the page.
  useEffect(() => {
    if (focusIndex < 0) return
    tabs.current[focusIndex]?.focus()
  }, [focusIndex])

  const tabId = (id: string) => `${generated}tab-${id}`
  const panelId = (id: string) => `${generated}panel-${id}`

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const current = focusIndex >= 0 ? focusIndex : selectedIndex
    const moved = nextFocusIndex({
      key: event.key,
      current,
      count: items.length,
      orientation,
      isDisabled: (index) => items[index]?.disabled === true,
    })
    if (!moved.handled) return
    event.preventDefault()
    setFocusIndex(moved.index)
    const target = items[moved.index]
    if (activation === 'automatic' && target !== undefined) setSelected(target.id)
  }

  return (
    <div className={cx('tg-tabs', `tg-tabs--${variant}`, `tg-tabs--${orientation}`, className)}>
      <div
        role="tablist"
        aria-label={ariaLabel}
        aria-orientation={orientation}
        className={cx('tg-tabs__list', stretch && 'tg-tabs__list--stretch')}
        onKeyDown={onKeyDown}
      >
        {items.map((item, index) => {
          const active = item.id === selected
          return (
            <button
              key={item.id}
              ref={(node) => {
                tabs.current[index] = node
              }}
              type="button"
              role="tab"
              id={tabId(item.id)}
              aria-selected={active}
              aria-controls={panels === undefined ? undefined : panelId(item.id)}
              aria-disabled={item.disabled === true ? true : undefined}
              tabIndex={active ? 0 : -1}
              className="tg-tabs__tab"
              onClick={() => {
                if (item.disabled === true) return
                setFocusIndex(index)
                setSelected(item.id)
              }}
            >
              {item.icon === undefined || item.icon === null ? null : (
                <span className="tg-tabs__icon" aria-hidden="true">
                  {item.icon}
                </span>
              )}
              <span className="tg-tabs__label">{item.label}</span>
              {item.badge === undefined || item.badge === null ? null : (
                <span className="tg-tabs__badge">{item.badge}</span>
              )}
            </button>
          )
        })}
      </div>
      {panels === undefined
        ? null
        : items.map((item) =>
            item.id === selected ? (
              <div
                key={item.id}
                role="tabpanel"
                id={panelId(item.id)}
                aria-labelledby={tabId(item.id)}
                tabIndex={0}
                className="tg-tabs__panel"
              >
                {panels[item.id]}
              </div>
            ) : null,
          )}
    </div>
  )
}
