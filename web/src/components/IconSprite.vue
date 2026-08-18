<script setup lang="ts">
defineProps<{
  name: 'rooms' | 'room-add' | 'members' | 'message' | 'send' | 'attach' | 'files' | 'search' | 'reply' | 'bell' | 'settings' | 'lock' | 'unread' | 'back'
  size?: number
}>()
</script>

<template>
  <svg :width="size || 18" :height="size || 18" aria-hidden="true">
    <use :href="`/icons/icon-sprite.svg#cr-icon-${name}`" />
  </svg>
</template>
