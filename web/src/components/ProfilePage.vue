<script setup lang="ts">
import { ref, watch } from 'vue'
import { ArrowLeft, ExternalLink, Save, UserRound } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Popover from 'primevue/popover'
import Textarea from 'primevue/textarea'
import EmojiPicker from './EmojiPicker.vue'
import { updateCurrentUser } from '../api'
import type { User } from '../types'

const props = defineProps<{ user: User; token: string }>()
const emit = defineEmits<{ back: []; updated: [user: User] }>()

const avatarEmoji = ref('')
const displayName = ref('')
const signature = ref('')
const homepage = ref('')
const saving = ref(false)
const error = ref('')
const saved = ref(false)
const avatarPopover = ref()

function selectAvatar(emoji: string): void {
  avatarEmoji.value = emoji
  avatarPopover.value?.hide()
}

watch(
  () => props.user,
  (user) => {
    avatarEmoji.value = user.avatar_emoji
    displayName.value = user.display_name
    signature.value = user.signature
    homepage.value = user.homepage
  },
  { immediate: true },
)

async function save(): Promise<void> {
  saving.value = true
  saved.value = false
  error.value = ''
  try {
    const user = await updateCurrentUser(props.token, {
      avatar_emoji: avatarEmoji.value,
      display_name: displayName.value,
      signature: signature.value,
      homepage: homepage.value,
    })
    emit('updated', user)
    saved.value = true
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '保存个人资料失败'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <main id="workspace-main" class="cr-page min-h-0 min-w-0 flex-1 overflow-y-auto">
    <header class="cr-page-header sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')"
        ><ArrowLeft :size="19"
      /></Button>
      <div>
        <h2 class="text-base font-semibold">个人资料</h2>
        <p class="mt-0.5 text-xs text-muted-color">@{{ user.username }}</p>
      </div>
    </header>

    <form autocomplete="on" class="cr-page-form mx-auto w-full max-w-2xl px-5 py-8 sm:px-8" @submit.prevent="save">
      <section class="cr-form-section pb-7">
        <div class="mb-4 flex items-center gap-2 text-sm font-semibold">
          <UserRound :size="18" class="text-primary" />头像
        </div>
        <div class="flex items-center gap-3">
          <Avatar
            v-if="avatarEmoji"
            :label="avatarEmoji"
            shape="circle"
            size="large"
            class="bg-primary-50! text-2xl!"
          />
          <Avatar
            v-else
            :label="user.username.slice(0, 1).toUpperCase()"
            shape="circle"
            size="large"
            class="bg-surface-200! text-surface-700!"
          />
          <Button type="button" outlined size="small" @click="avatarPopover.toggle($event)">选择表情</Button>
          <Button v-if="avatarEmoji" type="button" text severity="secondary" size="small" @click="avatarEmoji = ''"
            >清除</Button
          >
        </div>
        <Popover ref="avatarPopover">
          <EmojiPicker @select="selectAvatar" />
        </Popover>
      </section>

      <section class="cr-form-section space-y-5 py-7">
        <div>
          <label for="profile-display-name" class="mb-2 block text-sm font-medium">显示名称</label>
          <InputText
            id="profile-display-name"
            v-model="displayName"
            name="name"
            autocomplete="name"
            maxlength="48"
            fluid
          />
        </div>
        <div>
          <label for="profile-signature" class="mb-2 block text-sm font-medium">个性签名</label>
          <Textarea
            id="profile-signature"
            v-model="signature"
            name="profile-signature"
            autocomplete="off"
            maxlength="160"
            rows="3"
            auto-resize
            fluid
          />
          <small class="mt-1 block text-right text-muted-color">{{ signature.length }}/160</small>
        </div>
        <div>
          <label for="profile-homepage" class="mb-2 block text-sm font-medium">个人主页</label>
          <InputText
            id="profile-homepage"
            v-model="homepage"
            name="url"
            type="url"
            autocomplete="url"
            maxlength="240"
            placeholder="https://example.com…"
            fluid
          />
          <a
            v-if="user.homepage"
            :href="user.homepage"
            target="_blank"
            rel="noopener noreferrer"
            class="mt-2 inline-flex items-center gap-1 text-xs text-primary hover:underline"
          >
            查看主页 <ExternalLink :size="13" />
          </a>
        </div>
      </section>

      <Message v-if="error" severity="error" :closable="false" class="mt-5">{{ error }}</Message>
      <Message v-else-if="saved" severity="success" :closable="false" class="mt-5">个人资料已保存</Message>
      <div class="cr-form-footer flex justify-end pt-5">
        <Button type="submit" :loading="saving"><Save :size="17" /><span>保存资料</span></Button>
      </div>
    </form>
  </main>
</template>
