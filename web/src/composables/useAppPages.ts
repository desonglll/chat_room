import { ref, type Ref } from 'vue'
import type { User, Room } from '../types'

type AppPage = 'chat' | 'profile' | 'settings'

export function useAppPages(
  user: Ref<User | null>,
  selectedRoom: Ref<Room | null>,
  mobileView: Ref<'rooms' | 'chat'>,
  openAuthentication: () => void,
  openPreferences: () => void,
) {
  const activePage = ref<AppPage>('chat')

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

  function returnToChat(): void {
    activePage.value = 'chat'
    if (!selectedRoom.value) mobileView.value = 'rooms'
  }

  function requireAccount(action: () => void): void {
    if (user.value) action()
    else openAuthentication()
  }

  return { activePage, openProfile, openSettings, requireAccount, returnToChat }
}
