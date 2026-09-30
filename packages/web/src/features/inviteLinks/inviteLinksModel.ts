/**
 * Pure view logic for invite links: labels, expiry presets, the editor's validation, and how
 * a refusal reads on the landing page. No React, no fetch — tested directly.
 */
import type { InviteLink, InviteLinkInput, InviteRefusal } from '@tg/core'
import { MAX_INVITE_TITLE_CHARS, MAX_INVITE_USAGE_LIMIT } from '@tg/core'

/** Telegram's expiry choices; `null` seconds = never. */
export const EXPIRY_PRESETS: ReadonlyArray<{ id: string; label: string; seconds: number | null }> = [
  { id: 'hour', label: '1 小时', seconds: 3600 },
  { id: 'day', label: '1 天', seconds: 86_400 },
  { id: 'week', label: '1 周', seconds: 7 * 86_400 },
  { id: 'never', label: '永不过期', seconds: null },
]

/** Usage-limit choices; `null` = unlimited. */
export const LIMIT_PRESETS: ReadonlyArray<{ id: string; label: string; value: number | null }> = [
  { id: '1', label: '1', value: 1 },
  { id: '10', label: '10', value: 10 },
  { id: '50', label: '50', value: 50 },
  { id: '100', label: '100', value: 100 },
  { id: 'none', label: '不限', value: null },
]

export interface InviteLinkDraft {
  title: string
  /** An EXPIRY_PRESETS id, or `keep` when editing a link whose expiry stays as it is. */
  expiry: string
  /** Kept expiry (ISO) for `expiry === 'keep'`. */
  keptExpiresAt: string | null
  /** Digits as typed; empty = unlimited. */
  limit: string
  requiresApproval: boolean
}

export function emptyDraft(): InviteLinkDraft {
  return { title: '', expiry: 'never', keptExpiresAt: null, limit: '', requiresApproval: false }
}

export function draftFromLink(link: InviteLink): InviteLinkDraft {
  return {
    title: link.title,
    expiry: link.expires_at ? 'keep' : 'never',
    keptExpiresAt: link.expires_at,
    limit: link.usage_limit === null ? '' : String(link.usage_limit),
    requiresApproval: link.requires_approval,
  }
}

/** Validate a draft into the wire input, or return the Chinese error to show. */
export function draftToInput(draft: InviteLinkDraft, now: Date): InviteLinkInput | string {
  const title = draft.title.trim()
  if ([...title].length > MAX_INVITE_TITLE_CHARS) return `名称最多 ${MAX_INVITE_TITLE_CHARS} 个字符`
  let expiresAt: string | null = null
  if (draft.expiry === 'keep') {
    expiresAt = draft.keptExpiresAt
    if (expiresAt && Date.parse(expiresAt) <= now.getTime()) return '有效期已过，请重新选择'
  } else {
    const preset = EXPIRY_PRESETS.find((candidate) => candidate.id === draft.expiry)
    if (!preset) return '请选择有效期'
    expiresAt = preset.seconds === null ? null : new Date(now.getTime() + preset.seconds * 1000).toISOString()
  }
  let usageLimit: number | null = null
  if (!draft.requiresApproval && draft.limit.trim() !== '') {
    if (!/^\d+$/.test(draft.limit.trim())) return '人数上限必须是整数'
    usageLimit = Number(draft.limit.trim())
    if (usageLimit < 1 || usageLimit > MAX_INVITE_USAGE_LIMIT) return `人数上限为 1 – ${MAX_INVITE_USAGE_LIMIT}`
  }
  return { title, expires_at: expiresAt, usage_limit: usageLimit, requires_approval: draft.requiresApproval }
}

const pad = (value: number) => String(value).padStart(2, '0')

/** `2026-10-02 18:30` in the viewer's local time. */
export function formatWhen(iso: string): string {
  const date = new Date(iso)
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** A link's display name: its title, else what it is. */
export function linkName(link: InviteLink): string {
  if (link.title) return link.title
  return link.is_primary ? '主邀请链接' : '邀请链接'
}

/** The second line of a link row: usage, and why it no longer works or when it will stop. */
export function linkSummary(link: InviteLink): string {
  const parts: string[] = []
  parts.push(
    link.usage_limit === null ? `${link.usage_count} 人已加入` : `${link.usage_count}/${link.usage_limit} 人已加入`,
  )
  if (link.pending_count > 0) parts.push(`${link.pending_count} 个待审核`)
  switch (link.state) {
    case 'revoked':
      parts.push('已撤销')
      break
    case 'expired':
      parts.push('已过期')
      break
    case 'limit_reached':
      parts.push('名额已满')
      break
    default:
      if (link.expires_at) parts.push(`${formatWhen(link.expires_at)} 过期`)
      if (link.requires_approval) parts.push('需审核')
  }
  return parts.join(' · ')
}

/** The live and the revoked links, in server order, without the primary. */
export function partitionLinks(links: readonly InviteLink[]): {
  primary: InviteLink | null
  live: InviteLink[]
  revoked: InviteLink[]
} {
  const primary = links.find((link) => link.is_primary && link.state !== 'revoked') ?? null
  const rest = links.filter((link) => link !== primary)
  return {
    primary,
    live: rest.filter((link) => link.state !== 'revoked'),
    revoked: rest.filter((link) => link.state === 'revoked'),
  }
}

/** Whether the viewer may change this link (the server enforces the same rule). */
export function canChangeLink(link: InviteLink, viewerId: string | null, manageOthers: boolean): boolean {
  return link.is_primary || manageOthers || (viewerId !== null && link.creator_id === viewerId)
}

export const REFUSAL_COPY: Record<InviteRefusal, string> = {
  not_found: '邀请链接无效',
  expired: '邀请链接已过期',
  limit_reached: '邀请链接的名额已用完',
  revoked: '邀请链接已被撤销',
  banned: '你已被移出该群组，无法通过链接加入',
  locked: '该群组已被系统管理员锁定',
  internal: '暂时无法打开邀请链接，请稍后再试',
}
