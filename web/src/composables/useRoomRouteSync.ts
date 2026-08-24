import { watch, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { saveRoomPassword } from '../roomPasswordVault'
import type { Room } from '../types'

interface RoomRouteSyncOptions {
  authenticated: Ref<boolean>
  room: Ref<Room | null>
  password: Ref<string>
  rememberPasswords: () => boolean
}

export function shouldPromoteJoinRoute(online: boolean, routeName: unknown): boolean {
  return online && routeName === 'room-join'
}

export function useRoomRouteSync(options: RoomRouteSyncOptions): void {
  const route = useRoute()
  const router = useRouter()

  watch(options.authenticated, (online) => {
    const room = options.room.value
    if (online && room?.has_password) {
      saveRoomPassword(room.id, options.password.value, options.rememberPasswords())
    }
    // A refresh briefly makes the socket offline while it reconnects.  That
    // transport state must never turn an existing room URL into a join URL.
    if (!room || !shouldPromoteJoinRoute(online, route.name)) return

    const target = { name: 'room' as const, params: { id: room.id } }
    if (router.resolve(target).fullPath !== route.fullPath) {
      void router.replace(target).catch(() => {})
    }
  })
}
