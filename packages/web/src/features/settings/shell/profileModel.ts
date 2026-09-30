/**
 * The profile header and 我的账号 form, as pure functions (TG-110). The limits mirror
 * `src/accounts/user_handlers.rs::update_me`, so the form refuses what the server would 400.
 */
import type { UpdateProfilePayload, User } from '@tg/core'

export const PROFILE_LIMITS = { displayName: 48, signature: 160, homepage: 240 } as const

export interface ProfileHeaderModel {
  name: string
  handle: string
  avatarSrc: string | undefined
  initials: string | undefined
}

export function profileHeaderModel(user: User): ProfileHeaderModel {
  const uploaded = user.avatar_emoji.startsWith('/api/')
  return {
    name: user.display_name.trim() || user.username,
    handle: `@${user.username}`,
    avatarSrc: uploaded ? user.avatar_emoji : undefined,
    initials: uploaded ? undefined : user.avatar_emoji || undefined,
  }
}

export interface ProfileDraft {
  displayName: string
  signature: string
  homepage: string
}

export function profileDraftFrom(user: User): ProfileDraft {
  return { displayName: user.display_name, signature: user.signature, homepage: user.homepage }
}

export type ProfileErrors = Partial<Record<keyof ProfileDraft, string>>

const chars = (value: string) => [...value.trim()].length
// Control characters are refused by the server; built from code points so no literal is needed.
const CONTROL = new RegExp(`[${String.fromCharCode(0)}-${String.fromCharCode(31)}${String.fromCharCode(127)}]`)

export function validateProfileDraft(draft: ProfileDraft): ProfileErrors {
  const errors: ProfileErrors = {}
  if (chars(draft.displayName) > PROFILE_LIMITS.displayName)
    errors.displayName = `名称最多 ${PROFILE_LIMITS.displayName} 个字符`
  if (chars(draft.signature) > PROFILE_LIMITS.signature)
    errors.signature = `简介最多 ${PROFILE_LIMITS.signature} 个字符`
  else if (CONTROL.test(draft.signature.trim())) errors.signature = '简介不能包含换行或控制字符'
  const homepage = draft.homepage.trim()
  if (chars(homepage) > PROFILE_LIMITS.homepage) errors.homepage = `链接最多 ${PROFILE_LIMITS.homepage} 个字符`
  else if (homepage && !/^https?:\/\//.test(homepage)) errors.homepage = '链接需以 http:// 或 https:// 开头'
  return errors
}

/** Only the changed fields, trimmed; `null` when nothing changed. */
export function profilePatch(user: User, draft: ProfileDraft): UpdateProfilePayload | null {
  const patch: UpdateProfilePayload = {}
  if (draft.displayName.trim() !== user.display_name) patch.display_name = draft.displayName.trim()
  if (draft.signature.trim() !== user.signature) patch.signature = draft.signature.trim()
  if (draft.homepage.trim() !== user.homepage) patch.homepage = draft.homepage.trim()
  return Object.keys(patch).length > 0 ? patch : null
}
