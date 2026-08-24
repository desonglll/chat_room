import { useCallback, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { endpoints, SESSION_KEY } from '../../lib/api'
import type { AuthSession, User } from '../../types'
import { AuthContext } from './auth-context'

function initialSession() {
  const raw = localStorage.getItem(SESSION_KEY)
  if (!raw) return null
  try {
    const session = JSON.parse(raw) as AuthSession
    if (new Date(session.expires_at).getTime() <= Date.now()) {
      localStorage.removeItem(SESSION_KEY)
      return null
    }
    return session
  } catch {
    localStorage.removeItem(SESSION_KEY)
    return null
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<AuthSession | null>(initialSession)

  const persist = useCallback((next: AuthSession | null) => {
    setSession(next)
    if (next) localStorage.setItem(SESSION_KEY, JSON.stringify(next))
    else localStorage.removeItem(SESSION_KEY)
  }, [])

  const authenticate = useCallback(
    async (mode: 'login' | 'register', username: string, password: string) => {
      persist(await endpoints.authenticate(mode, username, password))
    },
    [persist],
  )

  const logout = useCallback(async () => {
    try {
      await endpoints.logout()
    } catch {
      // Local session removal must still succeed when the server is unreachable or already revoked.
    } finally {
      persist(null)
      sessionStorage.clear()
    }
  }, [persist])

  const updateUser = useCallback(
    (user: User) => {
      if (session) persist({ ...session, user })
    },
    [persist, session],
  )

  const value = useMemo(
    () => ({ session, user: session?.user ?? null, authenticate, logout, updateUser }),
    [authenticate, logout, session, updateUser],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}
