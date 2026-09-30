/**
 * Session and current user. Vanilla store (architecture.md §2): `packages/web` subscribes
 * with `useStore(authStore, …)`; nothing here knows React.
 */
import { createStore } from 'zustand/vanilla'
import type { AuthSession, User } from '../types'

export interface AuthState {
  session: AuthSession | null
  status: 'anonymous' | 'authenticating' | 'authenticated'
  setAuthenticating(): void
  setSession(session: AuthSession): void
  updateUser(user: User): void
  clearSession(): void
}

export const createAuthStore = () =>
  createStore<AuthState>()((set) => ({
    session: null,
    status: 'anonymous',
    setAuthenticating: () => set({ status: 'authenticating' }),
    setSession: (session) => set({ session, status: 'authenticated' }),
    updateUser: (user) => set((state) => (state.session ? { session: { ...state.session, user } } : {})),
    clearSession: () => set({ session: null, status: 'anonymous' }),
  }))

export type AuthStore = ReturnType<typeof createAuthStore>

/** Token for API calls; empty string while anonymous. */
export const selectToken = (state: AuthState): string => state.session?.token ?? ''

export const authStore = createAuthStore()
