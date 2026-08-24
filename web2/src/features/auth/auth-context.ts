import { createContext, useContext } from 'react'
import type { AuthSession, User } from '../../types'

export interface AuthContextValue {
  session: AuthSession | null
  user: User | null
  authenticate: (mode: 'login' | 'register', username: string, password: string) => Promise<void>
  logout: () => Promise<void>
  updateUser: (user: User) => void
}

export const AuthContext = createContext<AuthContextValue | null>(null)

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) throw new Error('useAuth must be used inside AuthProvider')
  return context
}
