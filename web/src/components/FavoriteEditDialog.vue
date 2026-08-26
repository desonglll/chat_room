<script setup lang="ts">
import { ref, watch } from 'vue'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Textarea from 'primevue/textarea'
import type { FavoriteItem } from '../types'

const props = defineProps<{
  item: FavoriteItem | null
  update: (id: string, version: number, title: string, content: string) => Promise<FavoriteItem>
}>()
const emit = defineEmits<{ close: []; success: [message: string]; error: [message: string] }>()
const title = ref('')
const content = ref('')
const busy = ref(false)

watch(
  () => props.item,
  (item) => {
    if (!item) return
    title.value = item.title
    content.value = item.content
  },
  { immediate: true },
)

async function submit(): Promise<void> {
  if (!props.item) return
  busy.value = true
  try {
    await props.update(props.item.id, props.item.version, title.value, content.value)
    emit('close')
    emit('success', '收藏已更新')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '更新收藏失败')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog
    :visible="Boolean(item)"
    modal
    header="编辑收藏"
    class="w-[min(92vw,560px)]"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <form class="space-y-4" @submit.prevent="submit">
      <div>
        <label for="favorite-edit-title" class="mb-2 block text-sm font-medium">标题</label>
        <InputText id="favorite-edit-title" v-model="title" maxlength="120" fluid />
      </div>
      <div>
        <label for="favorite-edit-content" class="mb-2 block text-sm font-medium">内容</label>
        <Textarea id="favorite-edit-content" v-model="content" maxlength="8000" rows="9" auto-resize fluid />
      </div>
      <p class="text-xs text-muted-color">版本 {{ item?.version }} · 保存时会检查其他协作者的修改</p>
      <div class="flex justify-end gap-2">
        <Button type="button" label="取消" severity="secondary" text @click="emit('close')" />
        <Button
          type="submit"
          label="保存"
          :loading="busy"
          :disabled="item?.kind === 'manual' && !title.trim() && !content.trim()"
        />
      </div>
    </form>
  </Dialog>
</template>
