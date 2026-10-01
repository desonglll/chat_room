/**
 * TG-404: the send button's long-press / right-click menu — «定时发送» (date-time picker,
 * lazily loaded) and «静默发送». Touch long-press, right click and Shift+F10 all come from
 * `ContextMenu`; the click that ends a long-press is swallowed so it does not also send.
 */
import { Suspense, lazy, useRef, useState, type ReactNode } from 'react'
import type { MenuItem } from '@tg/ui'
import { ContextMenu } from '@tg/ui'
import { scheduledActions } from '../scheduled/useScheduled'
import { scheduledErrorText } from '../scheduled/scheduledActions'
import type { ComposerController } from './composerController'
import { BellOffGlyph, CalendarGlyph } from './icons'
import { t } from '../../i18n/index'

const ScheduleDialog = lazy(() =>
  import('../scheduled/ScheduleDialog').then((module) => ({ default: module.ScheduleDialog })),
)

/** A click this soon after the menu opened is the tail of the long-press, not a send. */
const LONG_PRESS_CLICK_MS = 800

export interface SendMenuProps {
  chatId: string
  controller: ComposerController
  /** False while editing or offline: the button behaves as a plain button. */
  enabled: boolean
  children: ReactNode
}

export function SendMenu({ chatId, controller, enabled, children }: SendMenuProps) {
  const [scheduling, setScheduling] = useState(false)
  const openedAt = useRef(0)

  const items: MenuItem[] = [
    {
      id: 'schedule',
      label: t('w.composer.a0974f'),
      icon: <CalendarGlyph />,
      disabled: controller.schedulePayload() === null,
      onSelect: () => setScheduling(true),
    },
    { id: 'silent', label: t('w.composer.da1baa'), icon: <BellOffGlyph />, onSelect: () => controller.submitSilent() },
  ]

  const confirm = async (at: Date, silent: boolean) => {
    const payload = controller.schedulePayload()
    if (!payload) throw new Error(t('w.composer.fe87c8'))
    try {
      await scheduledActions.schedule(chatId, { ...payload, at, silent })
    } catch (caught) {
      throw new Error(scheduledErrorText(caught))
    }
    controller.scheduled()
  }

  return (
    <>
      <span
        className="tg-compose__send-menu"
        onClickCapture={(event) => {
          // Only the button's own click: menu rows are portalled out of this DOM subtree (but
          // still bubble through it in React), and they must stay clickable.
          const onButton = event.currentTarget.contains(event.target as Node)
          if (onButton && Date.now() - openedAt.current < LONG_PRESS_CLICK_MS) event.stopPropagation()
        }}
      >
        <ContextMenu
          items={items}
          disabled={!enabled}
          aria-label={t('w.composer.53ab68')}
          onOpenChange={(open) => {
            if (open) openedAt.current = Date.now()
          }}
        >
          {children}
        </ContextMenu>
      </span>
      {scheduling ? (
        <Suspense fallback={null}>
          <ScheduleDialog
            open
            title={t('w.composer.a0974f')}
            allowSilent
            onClose={() => setScheduling(false)}
            onConfirm={confirm}
          />
        </Suspense>
      ) : null}
    </>
  )
}
