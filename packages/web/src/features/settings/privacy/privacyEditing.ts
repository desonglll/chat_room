/**
 * Pure edits of one privacy rule, kept out of the components so they are testable without a
 * DOM. An account is in at most one list: adding it to one removes it from the other,
 * mirroring the server's one-exception-per-account rule.
 */
import type { PrivacyRule, PrivacyTier, PrivacyUser } from '@tg/core'

export type ExceptionList = 'allow' | 'deny'

const listField = (list: ExceptionList) => (list === 'allow' ? 'allow_users' : 'deny_users')

export function withTier(rule: PrivacyRule, tier: PrivacyTier): PrivacyRule {
  return { ...rule, tier }
}

export function withException(rule: PrivacyRule, list: ExceptionList, user: PrivacyUser): PrivacyRule {
  const other = list === 'allow' ? 'deny_users' : 'allow_users'
  const target = listField(list)
  if (rule[target].some((existing) => existing.id === user.id)) return rule
  return {
    ...rule,
    [other]: rule[other].filter((existing) => existing.id !== user.id),
    [target]: [...rule[target], user],
  }
}

export function withoutException(rule: PrivacyRule, list: ExceptionList, userId: string): PrivacyRule {
  const target = listField(list)
  return { ...rule, [target]: rule[target].filter((existing) => existing.id !== userId) }
}

export function replaceRule(rules: readonly PrivacyRule[], next: PrivacyRule): PrivacyRule[] {
  return rules.map((rule) => (rule.key === next.key ? next : rule))
}
