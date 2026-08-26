<script setup lang="ts">
import cytoscape, { type Core } from 'cytoscape'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import { getRoomKnowledgeGraph } from '../api'
import type { KnowledgeGraphFact, KnowledgeGraphSnapshot } from '../types'

const props = defineProps<{ open: boolean; roomId: string; roomName: string; token: string }>()
const emit = defineEmits<{ close: [] }>()

const canvas = ref<HTMLElement | null>(null)
const loading = ref(false)
const error = ref('')
const snapshot = ref<KnowledgeGraphSnapshot | null>(null)
const selectedFactId = ref('')
let graph: Core | null = null

const selectedFact = computed(
  () => snapshot.value?.facts.find((fact) => fact.id === selectedFactId.value) || snapshot.value?.facts[0] || null,
)

watch(
  () => [props.open, props.roomId] as const,
  ([open]) => {
    if (open) void loadGraph()
    else destroyGraph()
  },
  { immediate: true },
)

async function loadGraph(): Promise<void> {
  if (!props.roomId || loading.value) return
  loading.value = true
  error.value = ''
  try {
    snapshot.value = await getRoomKnowledgeGraph(props.roomId, props.token)
    selectedFactId.value = snapshot.value.facts[0]?.id || ''
    await nextTick()
    renderGraph(snapshot.value)
  } catch (caught) {
    snapshot.value = null
    destroyGraph()
    error.value = caught instanceof Error ? caught.message : '读取知识图谱失败'
  } finally {
    loading.value = false
  }
}

function renderGraph(value: KnowledgeGraphSnapshot): void {
  destroyGraph()
  if (!canvas.value || !value.nodes.length) return
  graph = cytoscape({
    container: canvas.value,
    elements: [
      ...value.nodes.map((node) => ({ data: { id: node.id, label: node.name } })),
      ...value.facts.map((fact) => ({
        data: {
          id: fact.id,
          source: fact.source_node_id,
          target: fact.target_node_id,
          label: fact.name,
        },
      })),
    ],
    layout: { name: 'cose', animate: false, fit: true, padding: 36, nodeRepulsion: () => 520_000 },
    style: [
      {
        selector: 'node',
        style: {
          'background-color': '#2563eb',
          'border-color': '#ffffff',
          'border-width': 2,
          color: '#172033',
          label: 'data(label)',
          'font-size': 11,
          'text-background-color': '#ffffff',
          'text-background-opacity': 0.92,
          'text-background-padding': '4px',
          'text-margin-y': 22,
          'text-max-width': '120px',
          'text-wrap': 'ellipsis',
          height: 34,
          width: 34,
        },
      },
      {
        selector: 'edge',
        style: {
          'curve-style': 'bezier',
          'line-color': '#94a3b8',
          'target-arrow-color': '#64748b',
          'target-arrow-shape': 'triangle',
          width: 1.5,
        },
      },
      {
        selector: 'edge:selected',
        style: { 'line-color': '#dc2626', 'target-arrow-color': '#dc2626', width: 3 },
      },
      { selector: 'node:selected', style: { 'background-color': '#dc2626' } },
    ],
  })
  graph.on('select', 'edge', (event) => {
    selectedFactId.value = event.target.id()
  })
  window.setTimeout(() => graph?.resize().fit(undefined, 36), 0)
}

function selectFact(fact: KnowledgeGraphFact): void {
  selectedFactId.value = fact.id
  graph?.getElementById(fact.id).select()
  const edge = graph?.getElementById(fact.id)
  if (edge && graph) graph.fit(edge.connectedNodes().union(edge), 80)
}

function destroyGraph(): void {
  graph?.destroy()
  graph = null
}

onBeforeUnmount(destroyGraph)
</script>

<template>
  <Dialog
    :visible="open"
    modal
    :header="`${roomName} · 知识图谱`"
    class="w-[min(96vw,1120px)]"
    :breakpoints="{ '700px': '100vw' }"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <div class="flex min-h-[min(72vh,720px)] flex-col">
      <div class="flex min-h-9 items-center justify-between gap-3 border-b border-surface-200 pb-3">
        <p class="text-xs text-muted-color">
          {{ snapshot ? `${snapshot.nodes.length} 个实体 · ${snapshot.facts.length} 条事实` : '' }}
          <span v-if="snapshot?.truncated"> · 已截取</span>
        </p>
        <Button text rounded severity="secondary" aria-label="刷新知识图谱" title="刷新" @click="loadGraph">
          <RefreshCw :size="17" :class="{ 'animate-spin motion-reduce:animate-none': loading }" />
        </Button>
      </div>

      <div v-if="error" class="grid flex-1 place-items-center text-sm text-danger">{{ error }}</div>
      <div v-else-if="loading && !snapshot" class="grid flex-1 place-items-center text-sm text-muted-color">加载中</div>
      <div
        v-else-if="snapshot && !snapshot.facts.length"
        class="grid flex-1 place-items-center text-sm text-muted-color"
      >
        暂无可用事实
      </div>
      <div v-else-if="snapshot" class="grid min-h-0 flex-1 md:grid-cols-[minmax(0,1fr)_300px]">
        <div ref="canvas" class="min-h-[52vh] bg-surface-50 md:min-h-0" aria-label="聊天室知识图谱" />
        <aside class="min-h-0 border-t border-surface-200 md:border-t-0 md:border-l">
          <div v-if="selectedFact" class="border-b border-surface-200 p-4">
            <p class="text-xs font-medium text-primary">{{ selectedFact.name }}</p>
            <p class="mt-2 break-words text-sm leading-6">{{ selectedFact.fact }}</p>
            <p class="mt-3 text-xs text-muted-color">来源消息 {{ selectedFact.episode_ids.length }} 条</p>
          </div>
          <ol class="max-h-56 divide-y divide-surface-100 overflow-y-auto md:max-h-[calc(72vh-180px)]">
            <li v-for="fact in snapshot.facts" :key="fact.id">
              <button
                type="button"
                class="w-full px-4 py-3 text-left text-xs hover:bg-surface-50 focus-visible:outline-2 focus-visible:outline-primary"
                :class="{ 'bg-primary-50': selectedFact?.id === fact.id }"
                @click="selectFact(fact)"
              >
                <strong class="block truncate">{{ fact.name }}</strong>
                <span class="mt-1 line-clamp-2 text-muted-color">{{ fact.fact }}</span>
              </button>
            </li>
          </ol>
        </aside>
      </div>
    </div>
  </Dialog>
</template>
