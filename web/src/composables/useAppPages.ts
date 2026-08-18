import { computed, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { User, Room } from '../types'

type AppPage = 'chat' | 'profile' | 'settings' | 'discover'

// The room itself only earns the plain /rooms/:id URL once actually
// connected — otherwise it's the join-gate, at its own /join URL, so
// refreshing there never looks like a silently-joined room.
function resolveTarget(page: AppPage, selectedRoom: Room | null, authenticated: boolean) {
  if (page === 'profile') return { name: 'profile' as const }
  if (page === 'settings') return { name: 'settings' as const }
  if (page === 'discover') return { name: 'discover' as const }
  if (!selectedRoom) return { name: 'home' as const }
  return authenticated
    ? { name: 'room' as const, params: { id: selectedRoom.id } }
    : { name: 'room-join' as const, params: { id: selectedRoom.id } }
}

export function useAppPages(
  user: Ref<User | null>,
  selectedRoom: Ref<Room | null>,
  mobileView: Ref<'rooms' | 'chat'>,
  authenticated: () => boolean,
  openAuthentication: () => void,
  openPreferences: () => void,
) {
  const route = useRoute()
  const router = useRouter()

  const activePage = computed<AppPage>({
    get: () => {
      if (route.name === 'profile') return 'profile'
      if (route.name === 'settings') return 'settings'
      if (route.name === 'discover') return 'discover'
      return 'chat'
    },
    set: (value) => {
      const target = resolveTarget(value, selectedRoom.value, authenticated())
      if (router.resolve(target).fullPath === route.fullPath) return
      void router.push(target).catch(() => {})
    },
  })

  function openProfile(): void {
    if (!user.value) return
    activePage.value = 'profile'
    mobileView.value = 'chat'
  }

  function openSettings(): void {
    if (!user.value) {
      openPreferences()
      return
    }
    activePage.value = 'settings'
    mobileView.value = 'chat'
  }

  function openDiscover(): void {
    activePage.value = 'discover'
    mobileView.value = 'chat'
  }

  function returnToChat(): void {
    activePage.value = 'chat'
    if (!selectedRoom.value) mobileView.value = 'rooms'
  }

  function requireAccount(action: () => void): void {
    if (user.value) action()
    else openAuthentication()
  }

  return { activePage, openProfile, openSettings, openDiscover, requireAccount, returnToChat }
}
