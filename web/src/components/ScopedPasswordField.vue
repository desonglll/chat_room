<script setup lang="ts">
import { computed } from 'vue'
import Password from 'primevue/password'
import { credentialInputProps, type CredentialScope } from '../credentialAutocomplete'

const props = withDefaults(defineProps<{
  modelValue: string
  inputId: string
  name: string
  scope: CredentialScope
  required?: boolean
  disabled?: boolean
}>(), {
  required: false,
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const inputProps = computed(() => credentialInputProps(props.scope))
</script>

<template>
  <Password
    :model-value="modelValue"
    :name="name"
    :input-id="inputId"
    :input-props="inputProps"
    :required="required"
    :disabled="disabled"
    :feedback="false"
    toggle-mask
    fluid
    @update:model-value="emit('update:modelValue', $event || '')"
  />
</template>
