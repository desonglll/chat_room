<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Ban, EllipsisVertical, ExternalLink, Save, Tag, UserMinus } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Menu from 'primevue/menu'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import { getUserProfile, setRoomNickname } from '../api'
import type { SocialUser, User } from '../types'
import AppAvatar from './AppAvatar.vue'

const props = defineProps<{
  open: boolean
  userId: string
  token: string
  roomId?: string
  currentUserId?: string
  contact: SocialUser | null
  setRemark: (userId: string, remark: string) => Promise<void>
}>()
const emit = defineEmits<{ close: []; removeFriend: []; blockUser: [] }>()

const profile = ref<User | null>(null)
const contactMenu = ref()
const loading = ref(false)
const error = ref('')
const actionError = ref('')
const nickname = ref('')
const savingNickname = ref(false)
const nicknameSaved = ref(false)
const editingRemark = ref(false)
const remark = ref('')
const savingRemark = ref(false)
const remarkSaved = ref(false)
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})
const isSelf = computed(() => Boolean(props.roomId) && props.userId === props.currentUserId)
const isContact = computed(() => props.contact?.id === props.userId)
const contactActions = [
  { label: '设置备注', icon: 'remark', command: () => (editingRemark.value = true) },
  { label: '删除好友', icon: 'remove', danger: true, command: removeContact },
  { label: '拉黑', icon: 'block', danger: true, command: blockContact },
]

watch(
  () => [props.open, props.userId, props.contact?.remark],
  async ([open, userId, contactRemark]) => {
    if (!open || !userId) return
    profile.value = null
    error.value = ''
    actionError.value = ''
    nickname.value = ''
    nicknameSaved.value = false
    editingRemark.value = false
    remark.value = (contactRemark as string | undefined) || ''
    remarkSaved.value = false
    loading.value = true
    try {
      profile.value = await getUserProfile(userId as string, props.token)
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : '读取用户资料失败'
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

function removeContact(): void {
  if (!profile.value || !window.confirm(`删除好友“${profile.value.display_name || profile.value.username}”？`)) return
  emit('removeFriend')
  emit('close')
}

function blockContact(): void {
  if (!profile.value || !window.confirm(`拉黑“${profile.value.display_name || profile.value.username}”？`)) return
  emit('blockUser')
  emit('close')
}

async function saveRemark(): Promise<void> {
  if (!profile.value) return
  savingRemark.value = true
  actionError.value = ''
  remarkSaved.value = false
  try {
    await props.setRemark(profile.value.id, remark.value.trim())
    editingRemark.value = false
    remarkSaved.value = true
  } catch (caught) {
    actionError.value = caught instanceof Error ? caught.message : '保存备注失败'
  } finally {
    savingRemark.value = false
  }
}

async function saveNickname(): Promise<void> {
  if (!props.roomId) return
  savingNickname.value = true
  nicknameSaved.value = false
  error.value = ''
  try {
    await setRoomNickname(props.roomId, props.token, nickname.value.trim())
    nicknameSaved.value = true
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '设置昵称失败'
  } finally {
    savingNickname.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal class="w-[min(92vw,400px)]" :draggable="false">
    <template #header>
      <div class="flex min-w-0 flex-1 items-center justify-between gap-3">
        <span class="font-semibold">用户资料</span>
        <div v-if="isContact">
          <Button
            text
            rounded
            severity="secondary"
            aria-label="联系人操作"
            title="联系人操作"
            aria-haspopup="menu"
            @click="contactMenu.toggle($event)"
          >
            <EllipsisVertical :size="19" />
          </Button>
          <Menu ref="contactMenu" :model="contactActions" :popup="true">
            <template #item="{ item, props: itemProps }">
              <button type="button" v-bind="itemProps.action" :class="{ 'text-danger!': item.danger }">
                <Tag v-if="item.icon === 'remark'" :size="17" />
                <UserMinus v-else-if="item.icon === 'remove'" :size="17" />
                <Ban v-else :size="17" />
                <span>{{ item.label }}</span>
              </button>
            </template>
          </Menu>
        </div>
      </div>
    </template>
    <div v-if="loading" class="flex flex-col items-center gap-3 py-4">
      <Skeleton shape="circle" size="4rem" />
      <Skeleton width="60%" height="1rem" />
      <Skeleton width="80%" height="0.8rem" />
    </div>
    <Message v-else-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
    <div v-else-if="profile" class="flex flex-col items-center gap-3 py-2 text-center">
      <AppAvatar
        :avatar="profile.avatar_emoji"
        :fallback="profile.username"
        :color-key="profile.id"
        class="size-16! text-3xl! text-white!"
      />
      <div>
        <strong class="block text-base">{{ profile.display_name || profile.username }}</strong>
        <span class="text-xs text-muted-color">@{{ profile.username }}</span>
      </div>
      <p v-if="profile.signature" class="text-sm text-surface-600">{{ profile.signature }}</p>
      <a
        v-if="profile.homepage"
        :href="profile.homepage"
        target="_blank"
        rel="noopener noreferrer"
        class="inline-flex items-center gap-1 text-xs text-primary hover:underline"
      >
        {{ profile.homepage }} <ExternalLink :size="12" />
      </a>

      <Message v-if="actionError" severity="error" size="small" :closable="false" class="w-full text-left">{{
        actionError
      }}</Message>
      <small v-else-if="remarkSaved" class="text-success">备注已保存</small>
      <form
        v-if="isContact && editingRemark"
        class="mt-2 w-full border-t border-surface-200 pt-4 text-left"
        @submit.prevent="saveRemark"
      >
        <label for="profile-friend-remark" class="mb-2 block text-sm font-medium">好友备注</label>
        <div class="flex gap-2">
          <InputText id="profile-friend-remark" v-model="remark" maxlength="64" class="min-w-0 flex-1" autofocus />
          <Button type="submit" :loading="savingRemark" aria-label="保存好友备注" title="保存备注">
            <Save :size="16" />
          </Button>
        </div>
      </form>

      <div v-if="isSelf" class="mt-2 w-full border-t border-surface-200 pt-4 text-left">
        <label for="roomNickname" class="mb-2 block text-sm font-medium">本房间昵称</label>
        <div class="flex gap-2">
          <InputText
            id="roomNickname"
            v-model="nickname"
            name="room-nickname"
            autocomplete="off"
            maxlength="48"
            placeholder="留空则显示原名称…"
            class="min-w-0 flex-1"
            fluid
          />
          <Button type="button" :loading="savingNickname" @click="saveNickname"><Save :size="16" /></Button>
        </div>
        <small v-if="nicknameSaved" class="mt-1 block text-success">已保存</small>
      </div>
    </div>
  </Dialog>
</template>
