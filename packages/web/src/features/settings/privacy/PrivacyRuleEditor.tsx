/** The editor of one privacy dimension: tier choice, footnote, and the exception lists. */
import { useState } from 'react'
import { RadioGroup } from '@tg/ui'
import { PRIVACY_TIERS, type PrivacyApi, type PrivacyRule, type PrivacyTier, type PrivacyUser } from '@tg/core'
import { ExceptionList } from './ExceptionList'
import { PRIVACY_KEY_COPY, PRIVACY_TIER_COPY, visibleExceptionLists } from './privacyCopy'
import { withException, withoutException, withTier, type ExceptionList as ListName } from './privacyEditing'
import { UserPicker } from './UserPicker'

export interface PrivacyRuleEditorProps {
  rule: PrivacyRule
  api: Pick<PrivacyApi, 'searchUsers'>
  saving: boolean
  onChange: (next: PrivacyRule) => void
}

const TIER_OPTIONS = PRIVACY_TIERS.map((tier) => ({ value: tier, label: PRIVACY_TIER_COPY[tier] }))

export function PrivacyRuleEditor({ rule, api, saving, onChange }: PrivacyRuleEditorProps) {
  const copy = PRIVACY_KEY_COPY[rule.key]
  const lists = visibleExceptionLists(rule.tier)
  const [picking, setPicking] = useState<ListName | null>(null)

  if (picking) {
    // Someone already on the other list may be picked: adding moves them.
    const listed = picking === 'allow' ? rule.allow_users : rule.deny_users
    return (
      <UserPicker
        api={api}
        title={picking === 'allow' ? copy.allowTitle : copy.denyTitle}
        excludeIds={listed.map((user) => user.id)}
        onCancel={() => setPicking(null)}
        onPick={(user: PrivacyUser) => {
          setPicking(null)
          onChange(withException(rule, picking, user))
        }}
      />
    )
  }

  return (
    <div className="tg-privacy__editor">
      <section className="tg-privacy__section">
        <RadioGroup
          name={`privacy-${rule.key}`}
          label={copy.question}
          options={TIER_OPTIONS}
          value={rule.tier}
          disabled={saving}
          onValueChange={(tier) => onChange(withTier(rule, tier as PrivacyTier))}
        />
        <p className="tg-privacy__footnote">{copy.footnote}</p>
      </section>
      {lists.deny ? (
        <ExceptionList
          title={copy.denyTitle}
          users={rule.deny_users}
          disabled={saving}
          onAdd={() => setPicking('deny')}
          onRemove={(id) => onChange(withoutException(rule, 'deny', id))}
        />
      ) : null}
      {lists.allow ? (
        <ExceptionList
          title={copy.allowTitle}
          users={rule.allow_users}
          disabled={saving}
          onAdd={() => setPicking('allow')}
          onRemove={(id) => onChange(withoutException(rule, 'allow', id))}
        />
      ) : null}
    </div>
  )
}
