/**
 * TG-505 privacy settings copy, Telegram's Chinese wording. Phone number is omitted on
 * purpose: accounts in this product have no phone numbers.
 */
import type { PrivacyKey, PrivacyRule, PrivacyTier } from '@tg/core'
import { t } from '../../../i18n/index'

export interface PrivacyKeyCopy {
  /** Row title in the privacy list and header of the editor. */
  title: string
  /** The question above the tier choice. */
  question: string
  /** Footnote under the tier choice. */
  footnote: string
  /** Heading of the allow list ("always …"). */
  allowTitle: string
  /** Heading of the deny list ("never …"). */
  denyTitle: string
}

export const PRIVACY_KEY_COPY: Record<PrivacyKey, PrivacyKeyCopy> = {
  last_seen: {
    get title() {
      return t('w.settings.450b4a')
    },
    get question() {
      return t('w.settings.2769bb')
    },
    get footnote() {
      return t('w.settings.87d1fd')
    },
    get allowTitle() {
      return t('w.settings.d19582')
    },
    get denyTitle() {
      return t('w.settings.6df240')
    },
  },
  profile_photo: {
    get title() {
      return t('w.settings.4ceeeb')
    },
    get question() {
      return t('w.settings.246986')
    },
    get footnote() {
      return t('w.settings.69db81')
    },
    get allowTitle() {
      return t('w.settings.d19582')
    },
    get denyTitle() {
      return t('w.settings.6df240')
    },
  },
  forwards: {
    get title() {
      return t('w.settings.d646f7')
    },
    get question() {
      return t('w.settings.32eb48')
    },
    get footnote() {
      return t('w.settings.67a0fe')
    },
    get allowTitle() {
      return t('w.settings.a0f208')
    },
    get denyTitle() {
      return t('w.settings.5a60f5')
    },
  },
  group_invites: {
    get title() {
      return t('w.settings.3c1a11')
    },
    get question() {
      return t('w.settings.8d3bfe')
    },
    get footnote() {
      return t('w.settings.73d64d')
    },
    get allowTitle() {
      return t('w.settings.a0f208')
    },
    get denyTitle() {
      return t('w.settings.5a60f5')
    },
  },
  voice_messages: {
    get title() {
      return t('w.settings.87053f')
    },
    get question() {
      return t('w.settings.816584')
    },
    get footnote() {
      return t('w.settings.59e166')
    },
    get allowTitle() {
      return t('w.settings.a0f208')
    },
    get denyTitle() {
      return t('w.settings.5a60f5')
    },
  },
}

export const PRIVACY_TIER_COPY: Record<PrivacyTier, string> = {
  get everybody() {
    return t('w.settings.f40c84')
  },
  get contacts() {
    return t('w.settings.e753a8')
  },
  get nobody() {
    return t('w.settings.bd4206')
  },
}

/** Which exception lists a tier makes meaningful (Telegram hides the other one). */
export function visibleExceptionLists(tier: PrivacyTier): { allow: boolean; deny: boolean } {
  return { allow: tier !== 'everybody', deny: tier !== 'nobody' }
}

/** The list row's value: "我的联系人 (-2, +1)" like Telegram's "My Contacts (-2, +1)". */
export function privacyRuleSummary(rule: PrivacyRule): string {
  const lists = visibleExceptionLists(rule.tier)
  const parts: string[] = []
  if (lists.deny && rule.deny_users.length > 0) parts.push(`-${rule.deny_users.length}`)
  if (lists.allow && rule.allow_users.length > 0) parts.push(`+${rule.allow_users.length}`)
  const tier = PRIVACY_TIER_COPY[rule.tier]
  return parts.length > 0 ? `${tier} (${parts.join(', ')})` : tier
}

export function privacyUserName(user: { username: string; display_name: string }): string {
  return user.display_name.trim() || user.username
}
