/**
 * TG-701: which lifecycle actions the viewer sees for a chat. Visibility only — the server
 * decides every request (`chat.info`, member management, ownership).
 */
import type { MembershipRole } from '@tg/core'

export interface LifecycleActions {
  edit: boolean
  invite: boolean
  leave: boolean
  delete: boolean
  removeMembers: boolean
}

export function lifecycleActions(role: MembershipRole | undefined, isPrivate: boolean): LifecycleActions {
  if (isPrivate || !role) return { edit: false, invite: false, leave: false, delete: false, removeMembers: false }
  const manager = role === 'owner' || role === 'admin'
  return {
    edit: manager,
    invite: manager,
    // The owner deletes instead (leaving would orphan the chat).
    leave: role !== 'owner',
    delete: role === 'owner',
    removeMembers: manager,
  }
}

/** Whether `actor` may remove a member with `target` role: never the owner, never themselves. */
export function canRemove(actor: MembershipRole | undefined, target: MembershipRole, self: boolean): boolean {
  if (self || target === 'owner') return false
  if (actor === 'owner') return true
  return actor === 'admin' && target === 'member'
}

export const TITLE_MAX = 64
export const DESCRIPTION_MAX = 255

export function profileError(title: string, description: string): string {
  if (!title.trim()) return 'w.lifecycle.titleRequired'
  if ([...title.trim()].length > TITLE_MAX) return 'w.lifecycle.titleTooLong'
  if ([...description].length > DESCRIPTION_MAX) return 'w.lifecycle.descriptionTooLong'
  return ''
}
