<script setup lang="ts">
import Button from 'primevue/button'
import Message from 'primevue/message'

defineProps<{ message: string }>()
defineEmits<{ retry: [] }>()
</script>

<template>
  <div v-if="message" class="cr-network-banner fixed top-3 z-50" role="alert">
    <Message severity="error" :closable="false">
      <div class="flex items-center gap-3">
        <span class="min-w-0 flex-1">{{ message }}</span>
        <Button label="重试" size="small" severity="danger" outlined @click="$emit('retry')" />
      </div>
    </Message>
  </div>
</template>
