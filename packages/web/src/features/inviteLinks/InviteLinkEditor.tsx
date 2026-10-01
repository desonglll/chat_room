/**
 * Create / edit an additional link: name, expiry presets, usage limit (presets or a typed
 * number), and "需要管理员审核" — which, as on the server, excludes a limit.
 */
import { useState } from 'react'
import type { InviteLink, InviteLinkInput } from '@tg/core'
import { MAX_INVITE_TITLE_CHARS } from '@tg/core'
import { Button, TextField, Toggle } from '@tg/ui'
import {
  draftFromLink,
  draftToInput,
  emptyDraft,
  EXPIRY_PRESETS,
  formatWhen,
  LIMIT_PRESETS,
  type InviteLinkDraft,
} from './inviteLinksModel'
import { t } from '../../i18n/index'

export interface InviteLinkEditorProps {
  /** Absent = create. */
  link?: InviteLink | undefined
  busy: boolean
  onSave(input: InviteLinkInput): void
  /** Injected clock for tests. */
  now?: () => Date
}

function Chip({ label, selected, onSelect }: { label: string; selected: boolean; onSelect(): void }) {
  return (
    <button type="button" className="tg-invite__chip" aria-pressed={selected} onClick={onSelect}>
      {label}
    </button>
  )
}

export function InviteLinkEditor({ link, busy, onSave, now = () => new Date() }: InviteLinkEditorProps) {
  const [draft, setDraft] = useState<InviteLinkDraft>(() => (link ? draftFromLink(link) : emptyDraft()))
  const [error, setError] = useState<string | null>(null)
  const patch = (change: Partial<InviteLinkDraft>) => setDraft((current) => ({ ...current, ...change }))

  const submit = () => {
    const input = draftToInput(draft, now())
    if (typeof input === 'string') {
      setError(input)
      return
    }
    setError(null)
    onSave(input)
  }

  return (
    <form
      className="tg-invite__editor"
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}
    >
      <div className="tg-invite__section">
        <TextField
          label={t('w.inviteLinks.7a65a9')}
          value={draft.title}
          maxLength={MAX_INVITE_TITLE_CHARS}
          hint={t('w.inviteLinks.48754b')}
          onChange={(event) => patch({ title: event.currentTarget.value })}
        />
      </div>
      <fieldset className="tg-invite__section tg-invite__fieldset">
        <legend className="tg-invite__section-title">{t('w.inviteLinks.222460')}</legend>
        <div className="tg-invite__chips">
          {draft.keptExpiresAt ? (
            <Chip
              label={t('w.inviteLinks.faaa55', formatWhen(draft.keptExpiresAt))}
              selected={draft.expiry === 'keep'}
              onSelect={() => patch({ expiry: 'keep' })}
            />
          ) : null}
          {EXPIRY_PRESETS.map((preset) => (
            <Chip
              key={preset.id}
              label={preset.label}
              selected={draft.expiry === preset.id}
              onSelect={() => patch({ expiry: preset.id })}
            />
          ))}
        </div>
      </fieldset>
      <div className="tg-invite__section">
        <Toggle
          label={t('w.inviteLinks.5eca12')}
          description={t('w.inviteLinks.ab8362')}
          checked={draft.requiresApproval}
          onCheckedChange={(checked) => patch({ requiresApproval: checked })}
        />
      </div>
      {draft.requiresApproval ? null : (
        <fieldset className="tg-invite__section tg-invite__fieldset">
          <legend className="tg-invite__section-title">{t('w.inviteLinks.2ab789')}</legend>
          <div className="tg-invite__chips">
            {LIMIT_PRESETS.map((preset) => (
              <Chip
                key={preset.id}
                label={preset.label}
                selected={draft.limit === (preset.value === null ? '' : String(preset.value))}
                onSelect={() => patch({ limit: preset.value === null ? '' : String(preset.value) })}
              />
            ))}
          </div>
          <TextField
            label={t('w.inviteLinks.228a7f')}
            inputMode="numeric"
            value={draft.limit}
            placeholder={t('w.inviteLinks.09c4fc')}
            onChange={(event) => patch({ limit: event.currentTarget.value })}
          />
        </fieldset>
      )}
      {error ? (
        <p className="tg-invite__error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="tg-invite__actions">
        <Button type="submit" variant="filled" loading={busy} disabled={busy}>
          {link ? t('w.inviteLinks.fadf24') : t('w.inviteLinks.286539')}
        </Button>
      </div>
    </form>
  )
}
