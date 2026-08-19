import type { InputHTMLAttributes } from 'vue'

export type CredentialScope = 'account-current' | 'account-new' | 'room-access' | 'room-new'

const autocompleteByScope: Record<CredentialScope, string> = {
  'account-current': 'section-user-account current-password',
  'account-new': 'section-user-account new-password',
  'room-access': 'section-chat-room-access one-time-code',
  'room-new': 'section-chat-room-settings one-time-code',
}

export function credentialInputProps(scope: CredentialScope): InputHTMLAttributes {
  return {
    autocomplete: autocompleteByScope[scope],
    minlength: scope === 'account-new' ? 8 : undefined,
    maxlength: 256,
  }
}
