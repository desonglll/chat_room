/**
 * Pure view logic for invite links: labels, expiry presets, the editor's validation, and how
 * a refusal reads on the landing page. No React, no fetch — tested directly.
 */
import type { InviteLink, InviteLinkInput, InviteRefusal } from '@tg/core'
import { MAX_INVITE_TITLE_CHARS, MAX_INVITE_USAGE_LIMIT } from '@tg/core'
import { t } from '../../i18n/index'

/** Telegram's expiry choices; `null` seconds = never. */
export const EXPIRY_PRESETS: ReadonlyArray<{ id: string; label: string; seconds: number | null }> = [
  {
    id: 'hour',
    get label() {
      return t('w.inviteLinks.c8fb1c')
    },
    seconds: 3600,
  },
  {
    id: 'day',
    get label() {
      return t('w.inviteLinks.11f478')
    },
    seconds: 86_400,
  },
  {
    id: 'week',
    get label() {
      return t('w.inviteLinks.03f320')
    },
    seconds: 7 * 86_400,
  },
  {
    id: 'never',
    get label() {
      return t('w.inviteLinks.0070ca')
    },
    seconds: null,
  },
]

/** Usage-limit choices; `null` = unlimited. */
export const LIMIT_PRESETS: ReadonlyArray<{ id: string; label: string; value: number | null }> = [
  { id: '1', label: '1', value: 1 },
  { id: '10', label: '10', value: 10 },
  { id: '50', label: '50', value: 50 },
  { id: '100', label: '100', value: 100 },
  {
    id: 'none',
    get label() {
      return t('w.inviteLinks.09c4fc')
    },
    value: null,
  },
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
  if ([...title].length > MAX_INVITE_TITLE_CHARS) return t('w.inviteLinks.5ef515', MAX_INVITE_TITLE_CHARS)
  let expiresAt: string | null = null
  if (draft.expiry === 'keep') {
    expiresAt = draft.keptExpiresAt
    if (expiresAt && Date.parse(expiresAt) <= now.getTime()) return t('w.inviteLinks.071ee8')
  } else {
    const preset = EXPIRY_PRESETS.find((candidate) => candidate.id === draft.expiry)
    if (!preset) return t('w.inviteLinks.e5053f')
    expiresAt = preset.seconds === null ? null : new Date(now.getTime() + preset.seconds * 1000).toISOString()
  }
  let usageLimit: number | null = null
  if (!draft.requiresApproval && draft.limit.trim() !== '') {
    if (!/^\d+$/.test(draft.limit.trim())) return t('w.inviteLinks.2c1f07')
    usageLimit = Number(draft.limit.trim())
    if (usageLimit < 1 || usageLimit > MAX_INVITE_USAGE_LIMIT) return t('w.inviteLinks.a1abf5', MAX_INVITE_USAGE_LIMIT)
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
  return link.is_primary ? t('w.inviteLinks.90b9ef') : t('w.inviteLinks.8a8f47')
}

/** The second line of a link row: usage, and why it no longer works or when it will stop. */
export function linkSummary(link: InviteLink): string {
  const parts: string[] = []
  parts.push(
    link.usage_limit === null
      ? t('w.inviteLinks.3e3d46', link.usage_count)
      : t('w.inviteLinks.75487b', link.usage_count, link.usage_limit),
  )
  if (link.pending_count > 0) parts.push(t('w.inviteLinks.988ec4', link.pending_count))
  switch (link.state) {
    case 'revoked':
      parts.push(t('w.inviteLinks.61063b'))
      break
    case 'expired':
      parts.push(t('w.inviteLinks.135437'))
      break
    case 'limit_reached':
      parts.push(t('w.inviteLinks.5addf4'))
      break
    default:
      if (link.expires_at) parts.push(t('w.inviteLinks.e137d1', formatWhen(link.expires_at)))
      if (link.requires_approval) parts.push(t('w.inviteLinks.422bd3'))
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
  get not_found() {
    return t('w.inviteLinks.4f4802')
  },
  get expired() {
    return t('w.inviteLinks.854fec')
  },
  get limit_reached() {
    return t('w.inviteLinks.a68055')
  },
  get revoked() {
    return t('w.inviteLinks.b06898')
  },
  get banned() {
    return t('w.inviteLinks.21c900')
  },
  get locked() {
    return t('w.inviteLinks.7a5eea')
  },
  get internal() {
    return t('w.inviteLinks.f068ed')
  },
}
