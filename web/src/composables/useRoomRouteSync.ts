import { watch, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storageSet } from '../browserStorage'
import type { Room } from '../types'

interface RoomRouteSyncOptions {
  authenticated: Ref<boolean>
  room: Ref<Room | null>
  password: Ref<string>
}

const passwordKey = (roomId: string) => `chat-room.password.${roomId}`

export function useRoomRouteSync(options: RoomRouteSyncOptions): void {
  const route = useRoute()
  const router = useRouter()

  watch(options.authenticated, (online) => {
    const room = options.room.value
    if (online && room?.has_password) {
      storageSet(window.sessionStorage, passwordKey(room.id), options.password.value)
    }
    if (!room || (route.name !== 'room' && route.name !== 'room-join')) return

    const target = online
      ? { name: 'room' as const, params: { id: room.id } }
      : { name: 'room-join' as const, params: { id: room.id } }
    if (router.resolve(target).fullPath !== route.fullPath) {
      void router.replace(target).catch(() => {})
    }
  })
}
