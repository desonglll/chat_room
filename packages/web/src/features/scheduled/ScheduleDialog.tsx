/**
 * TG-404 «定时发送» / «更改时间»: a native date-time picker plus Telegram-style quick picks.
 * The picker is `datetime-local` on purpose — it is the platform's own accessible control on
 * desktop and mobile, and it needs no new dependency.
 */
import { useEffect, useState } from 'react'
import { Button, Modal, TextField, Toggle } from '@tg/ui'
import {
  defaultScheduleTime,
  formatScheduleLabel,
  fromLocalInputValue,
  scheduleTimeError,
  toLocalInputValue,
} from './scheduleTime'
import { t } from '../../i18n/index'

export interface ScheduleDialogProps {
  open: boolean
  title: string
  /** Pre-selected time; defaults to one hour from now. */
  initial?: Date | undefined
  /** Show the «静默发送» switch (new schedules only). */
  allowSilent?: boolean | undefined
  onClose(): void
  /** Resolve to close; reject with a user-facing message to keep the dialog open. */
  onConfirm(at: Date, silent: boolean): Promise<void>
}

function quickPicks(now: Date): Array<{ label: string; at: Date }> {
  const inHour = defaultScheduleTime(now)
  const tonight = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 21, 0)
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9, 0)
  const picks = [{ label: t('w.scheduled.9471dd'), at: inHour }]
  if (tonight.getTime() > inHour.getTime()) picks.push({ label: t('w.scheduled.e6faa6'), at: tonight })
  picks.push({ label: t('w.scheduled.2a0eb8'), at: tomorrow })
  return picks
}

export function ScheduleDialog({ open, title, initial, allowSilent = false, onClose, onConfirm }: ScheduleDialogProps) {
  const [value, setValue] = useState(() => toLocalInputValue(initial ?? defaultScheduleTime(new Date())))
  const [silent, setSilent] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (!open) return
    setValue(toLocalInputValue(initial ?? defaultScheduleTime(new Date())))
    setSilent(false)
    setError(null)
  }, [open, initial])

  const now = new Date()
  const at = fromLocalInputValue(value)
  const invalid = scheduleTimeError(at, now)

  const confirm = () => {
    if (invalid || !at) {
      setError(invalid)
      return
    }
    setBusy(true)
    onConfirm(at, silent)
      .then(onClose)
      .catch((caught: unknown) => setError(caught instanceof Error ? caught.message : String(caught)))
      .finally(() => setBusy(false))
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      size="sm"
      className="tg-schedule"
      footer={
        <>
          <Button variant="text" onClick={onClose}>
            {t('w.scheduled.4d0b46')}
          </Button>
          <Button onClick={confirm} loading={busy} disabled={busy || invalid !== null}>
            {at && !invalid ? formatScheduleLabel(at, now) : t('w.scheduled.a0974f')}
          </Button>
        </>
      }
    >
      <div className="tg-schedule__picks" role="group" aria-label={t('w.scheduled.c49358')}>
        {quickPicks(now).map((pick) => (
          <Button
            key={pick.label}
            size="sm"
            variant={toLocalInputValue(pick.at) === value ? 'tonal' : 'text'}
            onClick={() => setValue(toLocalInputValue(pick.at))}
          >
            {pick.label}
          </Button>
        ))}
      </div>
      <TextField
        type="datetime-local"
        label={t('w.scheduled.98c64d')}
        value={value}
        min={toLocalInputValue(now)}
        fullWidth
        error={error ?? undefined}
        onChange={(event) => {
          setValue(event.target.value)
          setError(null)
        }}
      />
      {allowSilent ? (
        <Toggle
          label={t('w.scheduled.da1baa')}
          description={t('w.scheduled.8b7652')}
          checked={silent}
          onCheckedChange={setSilent}
        />
      ) : null}
    </Modal>
  )
}
