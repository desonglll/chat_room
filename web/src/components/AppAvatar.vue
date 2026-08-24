<script setup lang="ts">
import { computed } from 'vue'
import Avatar from 'primevue/avatar'
import { avatarColor } from '../avatarColor'

defineOptions({ inheritAttrs: false })
const props = defineProps<{ avatar?: string; fallback: string; colorKey?: string }>()
const image = computed(() => (props.avatar?.startsWith('/api/') ? props.avatar : undefined))
const label = computed(() => (image.value ? undefined : props.avatar || props.fallback.slice(0, 1).toUpperCase()))
</script>

<template>
  <Avatar
    v-bind="$attrs"
    :image="image"
    :label="label"
    shape="circle"
    :style="image ? undefined : { backgroundColor: avatarColor(colorKey || fallback) }"
  />
</template>
