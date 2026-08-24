import { Navigate, Outlet, useLocation } from 'react-router'
import { useAuth } from './auth-context'

export function RequireAuth() {
  const { session } = useAuth()
  const location = useLocation()
  return session ? <Outlet /> : <Navigate to="/auth" replace state={{ from: location.pathname }} />
}
