/**
 * TG-505 privacy settings copy, Telegram's Chinese wording. Phone number is omitted on
 * purpose: accounts in this product have no phone numbers.
 */
import type { PrivacyKey, PrivacyRule, PrivacyTier } from '@tg/core'

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
    title: '最后上线时间',
    question: '谁可以看到我的最后上线时间和在线状态？',
    footnote:
      '如果你不分享自己的最后上线时间，你也将无法看到他人的最后上线时间。对方看到的将是模糊的时间（如“最近上线”），且只按天更新。',
    allowTitle: '总是分享给',
    denyTitle: '永不分享给',
  },
  profile_photo: {
    title: '头像',
    question: '谁可以看到我的头像？',
    footnote: '无权查看的人会看到由你名字首字生成的默认头像。',
    allowTitle: '总是分享给',
    denyTitle: '永不分享给',
  },
  forwards: {
    title: '转发消息',
    question: '转发我的消息时，谁可以显示我的名字？',
    footnote: '无权显示的人转发你的消息时，署名将显示为“隐藏的账号”。',
    allowTitle: '总是允许',
    denyTitle: '永不允许',
  },
  group_invites: {
    title: '群组邀请',
    question: '谁可以邀请我加入群组？',
    footnote: '无权邀请的人无法把你加入群组。',
    allowTitle: '总是允许',
    denyTitle: '永不允许',
  },
  voice_messages: {
    title: '语音消息',
    question: '谁可以在私聊中给我发送语音消息？',
    footnote: '无权发送的人在与你的私聊中无法发送语音消息。',
    allowTitle: '总是允许',
    denyTitle: '永不允许',
  },
}

export const PRIVACY_TIER_COPY: Record<PrivacyTier, string> = {
  everybody: '所有人',
  contacts: '我的联系人',
  nobody: '没有人',
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
