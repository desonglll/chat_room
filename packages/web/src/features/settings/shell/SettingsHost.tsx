/**
 * The shell's mount point for the settings panel (TG-110). The panel module — and with it
 * every settings page — is fetched on the first open only; after that it stays mounted so
 * its close slide can play.
 */
import { lazy, Suspense, useState } from 'react'
import { useStore } from 'zustand/react'
import { settingsNavigation } from './settingsNavigation'

const SettingsPanel = lazy(() => import('./SettingsPanel'))

export function SettingsHost() {
  const open = useStore(settingsNavigation, (state) => state.open)
  const [loaded, setLoaded] = useState(open)
  if (open && !loaded) setLoaded(true)
  if (!loaded) return null
  return (
    <Suspense fallback={null}>
      <SettingsPanel />
    </Suspense>
  )
}
