/**
 * Settings → 我的账号 (TG-110): name, bio and link, saved through the existing
 * `PATCH /api/users/me`. The username is shown read-only (there is no rename endpoint). The
 * saved `User` goes back into `authStore`, so the settings header and every other reader of
 * the session follow at once. TG-511 adds the avatar page beside this one.
 */
import { useState, type FormEvent } from 'react'
import { authStore, selectToken, updateCurrentUser } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { useStore } from 'zustand/react'
import { apiClient } from '../../../../app/client'
import type { SettingsPageProps } from '../settingsRegistry'
import {
  PROFILE_LIMITS,
  profileDraftFrom,
  profilePatch,
  validateProfileDraft,
  type ProfileDraft,
} from '../profileModel'
import { t } from '../../../../i18n/index'

export function AccountPage({ onBack }: SettingsPageProps) {
  const token = useStore(authStore, selectToken)
  const user = useStore(authStore, (state) => state.session?.user ?? null)
  const [draft, setDraft] = useState<ProfileDraft | null>(user ? profileDraftFrom(user) : null)
  const [saving, setSaving] = useState(false)
  const [failure, setFailure] = useState('')
  if (!user || !draft) return null

  const errors = validateProfileDraft(draft)
  const patch = profilePatch(user, draft)
  const invalid = Object.keys(errors).length > 0

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!patch || invalid || saving) return
    setSaving(true)
    setFailure('')
    try {
      const saved = await updateCurrentUser(apiClient, token, patch)
      authStore.getState().updateUser(saved)
      setDraft(profileDraftFrom(saved))
      onBack()
    } catch {
      setFailure(t('w.settings.f6d210'))
    } finally {
      setSaving(false)
    }
  }

  const field = (key: keyof ProfileDraft) => (value: string) => setDraft({ ...draft, [key]: value })

  return (
    <form className="tg-settings-account" onSubmit={(event) => void submit(event)} noValidate>
      <div className="tg-settings__group">
        <TextField
          label={t('w.settings.1be7ae')}
          value={draft.displayName}
          maxLength={PROFILE_LIMITS.displayName}
          placeholder={user.username}
          error={errors.displayName}
          fullWidth
          onChange={(event) => field('displayName')(event.target.value)}
        />
        <TextField
          label={t('w.settings.5ea2e0')}
          value={draft.signature}
          maxLength={PROFILE_LIMITS.signature}
          hint={t('w.settings.2e2912')}
          error={errors.signature}
          fullWidth
          onChange={(event) => field('signature')(event.target.value)}
        />
        <TextField
          label={t('w.settings.715022')}
          type="url"
          value={draft.homepage}
          maxLength={PROFILE_LIMITS.homepage}
          placeholder="https://"
          error={errors.homepage}
          fullWidth
          onChange={(event) => field('homepage')(event.target.value)}
        />
      </div>
      <div className="tg-settings__group">
        <h3 className="tg-settings__group-title">{t('w.settings.a1aaf3')}</h3>
        <p className="tg-settings__value">@{user.username}</p>
        <p className="tg-settings__hint">{t('w.settings.57d67b')}</p>
      </div>
      {failure ? (
        <p className="tg-settings__error" role="alert">
          {failure}
        </p>
      ) : null}
      <div className="tg-settings__actions">
        <Button type="submit" disabled={!patch || invalid} loading={saving} fullWidth>
          {t('w.settings.fadf24')}
        </Button>
      </div>
    </form>
  )
}

export default AccountPage
