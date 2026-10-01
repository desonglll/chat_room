/**
 * Pure rules of the chat administration UI (TG-201). The server re-decides everything; these
 * only decide what to *show*: which entry points, which checkboxes, which copy.
 */
import type { ChatMemberEntry, ChatMembership, ChatPermissionsView, ChatType, PermissionDescriptor } from '@tg/core'
import { ADMIN_ASSIGNABLE_KEYS, MEMBER_TOGGLEABLE_KEYS } from '@tg/core'
import { t } from '../../i18n/index'

export const CHAT_TYPE_LABEL: Record<ChatType, string> = {
  get private() {
    return t('w.chatAdmin.3adfdb')
  },
  get group() {
    return t('w.chatAdmin.4260ca')
  },
  get supergroup() {
    return t('w.chatAdmin.a06237')
  },
  get channel() {
    return t('w.chatAdmin.b76dfd')
  },
}

/** The line under the chat type: Telegram upgrades automatically, never by a button. */
export function chatTypeNote(type: ChatType): string {
  if (type === 'group') return t('w.chatAdmin.7eaea5')
  if (type === 'supergroup') return t('w.chatAdmin.001782')
  return ''
}

const has = (view: ChatPermissionsView | null, key: string) => view?.my_permissions.includes(key) ?? false

/** What the viewer may open in the admin panel. */
export interface AdminCapabilities {
  /** Change group defaults and restrict members. */
  ban: boolean
  /** Appoint administrators (members.promote). */
  promote: boolean
  /** Edit or dismiss an existing administrator: the owner (members.roles). */
  manageAdmins: boolean
  /** Any of the above: the entry point is shown. */
  any: boolean
}

export function adminCapabilities(view: ChatPermissionsView | null): AdminCapabilities {
  const ban = has(view, 'members.ban')
  const promote = has(view, 'members.promote')
  const manageAdmins = has(view, 'members.roles')
  return { ban, promote, manageAdmins, any: ban || promote || manageAdmins }
}

export function labelOf(registry: readonly PermissionDescriptor[], key: string): string {
  return registry.find((entry) => entry.key === key)?.label ?? key
}

/** One checkbox of an editor. */
export interface PermissionOption {
  key: string
  label: string
  checked: boolean
  /** Visible but not changeable (a right the actor does not hold, or a missing prerequisite). */
  disabled: boolean
}

/** Content kinds that need `message.send` too — unchecked sending greys them out. */
const NEEDS_SEND = new Set(['message.send_media', 'message.send_sticker', 'message.send_poll', 'message.embed_link'])

/** The member-permission checkboxes, for group defaults (`allowed` = on) or a member. */
export function memberOptions(
  registry: readonly PermissionDescriptor[],
  allowed: ReadonlySet<string>,
): PermissionOption[] {
  return MEMBER_TOGGLEABLE_KEYS.map((key) => ({
    key,
    label: labelOf(registry, key),
    checked: allowed.has(key) && (!NEEDS_SEND.has(key) || allowed.has('message.send')),
    disabled: NEEDS_SEND.has(key) && !allowed.has('message.send'),
  }))
}

/** Toggle one member permission; switching sending off switches every content kind off. */
export function toggleMemberPermission(allowed: ReadonlySet<string>, key: string, on: boolean): Set<string> {
  const next = new Set(allowed)
  if (on) next.add(key)
  else next.delete(key)
  if (key === 'message.send' && !on) for (const dependent of NEEDS_SEND) next.delete(dependent)
  return next
}

/** The allowed set a member currently has: the group defaults minus their restrictions. */
export function memberAllowed(defaults: readonly string[], entry: ChatMemberEntry): Set<string> {
  const denied = new Set((entry.restrictions ?? []).map((restriction) => restriction.permission_key))
  return new Set(defaults.filter((key) => !denied.has(key)))
}

/** What to send as `denied_permissions`: every default the editor switched off. */
export function deniedFrom(defaults: readonly string[], allowed: ReadonlySet<string>): string[] {
  return MEMBER_TOGGLEABLE_KEYS.filter((key) => defaults.includes(key) && !allowed.has(key))
}

/**
 * The administrator checkboxes. The owner may grant anything; an administrator only what
 * they hold themselves (the server enforces the same rule).
 */
export function adminOptions(
  registry: readonly PermissionDescriptor[],
  selected: ReadonlySet<string>,
  actor: ChatPermissionsView,
): PermissionOption[] {
  const owner = actor.my_permissions.includes('members.roles')
  return ADMIN_ASSIGNABLE_KEYS.map((key) => ({
    key,
    label: labelOf(registry, key),
    checked: selected.has(key),
    disabled: !owner && !actor.my_permissions.includes(key),
  }))
}

/** A new administrator's preselection: everything the actor may grant except appointing. */
export function defaultAdminSelection(actor: ChatPermissionsView): Set<string> {
  const owner = actor.my_permissions.includes('members.roles')
  return new Set(
    ADMIN_ASSIGNABLE_KEYS.filter(
      (key) =>
        key !== 'members.promote' &&
        key !== 'chat.anonymous' &&
        key !== 'message.post' &&
        key !== 'chat.call' &&
        (owner || actor.my_permissions.includes(key)),
    ),
  )
}

/** Restriction lengths offered as presets, Telegram-style. `null` = until lifted. */
export const UNTIL_PRESETS: ReadonlyArray<{ id: string; label: string; seconds: number | null }> = [
  {
    id: 'hour',
    get label() {
      return t('w.chatAdmin.c8fb1c')
    },
    seconds: 3600,
  },
  {
    id: 'day',
    get label() {
      return t('w.chatAdmin.11f478')
    },
    seconds: 86_400,
  },
  {
    id: 'week',
    get label() {
      return t('w.chatAdmin.03f320')
    },
    seconds: 604_800,
  },
  {
    id: 'forever',
    get label() {
      return t('w.chatAdmin.409752')
    },
    seconds: null,
  },
]

export function untilFromPreset(presetId: string, now: number): string | null {
  const preset = UNTIL_PRESETS.find((entry) => entry.id === presetId)
  if (!preset || preset.seconds === null) return null
  return new Date(now + preset.seconds * 1000).toISOString()
}

/** "直到 10月2日 14:30" / "永久". */
export function untilText(until: string | null | undefined): string {
  if (!until) return t('w.chatAdmin.409752')
  const date = new Date(until)
  const pad = (value: number) => String(value).padStart(2, '0')
  return t('w.chatAdmin.ee5028', date.getMonth() + 1, date.getDate(), pad(date.getHours()), pad(date.getMinutes()))
}

export const memberName = (entry: Pick<ChatMemberEntry, 'nickname' | 'display_name' | 'username'>): string =>
  entry.nickname || entry.display_name || entry.username

/** The badge after a name: the admin title, else 所有者 / 管理员. */
export function roleBadge(entry: ChatMemberEntry): string {
  if (entry.custom_title) return entry.custom_title
  if (entry.role === 'owner') return t('w.chatAdmin.ff3beb')
  if (entry.role === 'admin') return t('w.chatAdmin.ef84e7')
  return ''
}

/** A roster entry as the older `ChatMembership` shape (for TG-106's member tab). */
export function toMembership(entry: ChatMemberEntry): ChatMembership {
  return {
    user_id: entry.user_id,
    username: entry.username,
    avatar_emoji: entry.avatar_emoji,
    nickname: entry.nickname,
    role: entry.role,
    status: 'active',
    requested_at: entry.joined_at ?? '',
    joined_at: entry.joined_at,
  }
}
