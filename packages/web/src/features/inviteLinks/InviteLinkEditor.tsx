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
          label="链接名称（可选）"
          value={draft.title}
          maxLength={MAX_INVITE_TITLE_CHARS}
          hint="只有管理员能看到名称"
          onChange={(event) => patch({ title: event.currentTarget.value })}
        />
      </div>
      <fieldset className="tg-invite__section tg-invite__fieldset">
        <legend className="tg-invite__section-title">有效期</legend>
        <div className="tg-invite__chips">
          {draft.keptExpiresAt ? (
            <Chip
              label={`保持（${formatWhen(draft.keptExpiresAt)}）`}
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
          label="需要管理员审核"
          description="通过此链接的人需要管理员批准后才能加入；审核链接不设人数上限。"
          checked={draft.requiresApproval}
          onCheckedChange={(checked) => patch({ requiresApproval: checked })}
        />
      </div>
      {draft.requiresApproval ? null : (
        <fieldset className="tg-invite__section tg-invite__fieldset">
          <legend className="tg-invite__section-title">人数上限</legend>
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
            label="自定义人数"
            inputMode="numeric"
            value={draft.limit}
            placeholder="不限"
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
          {link ? '保存' : '创建链接'}
        </Button>
      </div>
    </form>
  )
}
