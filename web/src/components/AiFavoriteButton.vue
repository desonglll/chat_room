<script setup lang="ts">
import { ref } from 'vue'
import { Bookmark, BookmarkCheck } from 'lucide-vue-next'
import Button from 'primevue/button'
import { useToast } from 'primevue/usetoast'
import { aiFavoriteContent, aiFavoriteTitle } from '../aiFavorite'
import type { AiThreadMessage, FavoriteItem } from '../types'

const props = defineProps<{
  message: AiThreadMessage
  roomTitle: string
  save: (title: string, content: string) => Promise<FavoriteItem>
}>()
const saving = ref(false)
const saved = ref(false)
const toast = useToast()

async function save(): Promise<void> {
  if (saving.value || saved.value) return
  saving.value = true
  try {
    await props.save(aiFavoriteTitle(props.roomTitle), aiFavoriteContent(props.message))
    saved.value = true
    toast.add({ severity: 'success', summary: 'AI 回答已收藏', life: 2400 })
  } catch (caught) {
    toast.add({
      severity: 'error',
      summary: caught instanceof Error ? caught.message : '收藏 AI 回答失败',
      life: 3200,
    })
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <Button
    v-if="message.status === 'completed' && message.content.trim()"
    text
    rounded
    severity="secondary"
    size="small"
    class="ml-auto size-8! p-0!"
    :loading="saving"
    :disabled="saved"
    :aria-label="saved ? 'AI 回答已收藏' : '收藏 AI 回答'"
    :title="saved ? '已收藏' : '收藏 AI 回答'"
    @click="save"
  >
    <BookmarkCheck v-if="saved" :size="15" class="text-primary" />
    <Bookmark v-else-if="!saving" :size="15" />
  </Button>
</template>
