<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  Bell,
  Blocks,
  Braces,
  ChevronDown,
  ChevronRight,
  CircleUserRound,
  Code2,
  Files,
  GitBranch,
  Search,
  Settings,
  SplitSquareHorizontal,
  TestTube2,
  X,
} from 'lucide-vue-next'
import { createIdleDisguiseController } from '../idleDisguise'
import { VSCODE_SAMPLE_LINES } from '../vscodeDisguiseCode'

const ACTIVITY_EVENTS = ['keydown', 'pointerdown', 'pointermove', 'touchstart', 'wheel'] as const
const props = defineProps<{ enabled: boolean }>()
const visible = ref(false)
let mounted = false
const controller = createIdleDisguiseController((active) => {
  visible.value = active
})

function handleActivity(event: Event): void {
  const wasVisible = visible.value
  controller.activity()
  const privacyLockVisible = document.querySelector('[data-privacy-lock-root]')
  if (wasVisible && event instanceof KeyboardEvent && !privacyLockVisible) {
    event.preventDefault()
    event.stopImmediatePropagation()
  }
}

watch(
  () => props.enabled,
  (enabled) => {
    if (mounted) controller.setEnabled(enabled)
  },
)

onMounted(() => {
  mounted = true
  for (const eventName of ACTIVITY_EVENTS) window.addEventListener(eventName, handleActivity, true)
  controller.setEnabled(props.enabled)
})

onBeforeUnmount(() => {
  mounted = false
  for (const eventName of ACTIVITY_EVENTS) window.removeEventListener(eventName, handleActivity, true)
  controller.stop()
})
</script>

<template>
  <Teleport to="body">
    <Transition name="disguise-fade">
      <section v-if="visible" class="vs-shell" aria-hidden="true" data-testid="vscode-disguise">
        <header class="vs-titlebar">
          <div class="vs-window-controls"><i></i><i></i><i></i></div>
          <nav class="vs-menu">
            <span>File</span><span>Edit</span><span>Selection</span><span>View</span><span>Go</span><span>Run</span
            ><span>Terminal</span><span>Help</span>
          </nav>
          <div class="vs-title"><Code2 :size="14" /> workspace — Visual Studio Code</div>
          <div class="vs-layout-actions"><SplitSquareHorizontal :size="15" /><X :size="16" /></div>
        </header>

        <div class="vs-workbench">
          <aside class="vs-activitybar">
            <div class="vs-activity-icons">
              <span class="active"><Files :size="24" /></span>
              <span><Search :size="23" /></span>
              <span><GitBranch :size="23" /><b>2</b></span>
              <span><TestTube2 :size="23" /></span>
              <span><Blocks :size="23" /></span>
            </div>
            <div class="vs-activity-icons">
              <span><CircleUserRound :size="23" /></span>
              <span><Settings :size="23" /></span>
            </div>
          </aside>

          <aside class="vs-explorer">
            <div class="vs-explorer-title">EXPLORER <span>•••</span></div>
            <div class="vs-tree-heading"><ChevronDown :size="14" /><strong>WORKSPACE</strong></div>
            <div class="vs-tree-row"><ChevronDown :size="14" /><span class="folder">src</span></div>
            <div class="vs-tree-row nested selected">
              <Braces :size="14" class="ts-icon" /><span>server.ts</span><i>M</i>
            </div>
            <div class="vs-tree-row nested"><Braces :size="14" class="ts-icon" /><span>router.ts</span></div>
            <div class="vs-tree-row nested"><Braces :size="14" class="ts-icon" /><span>types.ts</span></div>
            <div class="vs-tree-row"><ChevronRight :size="14" /><span class="folder">tests</span></div>
            <div class="vs-tree-row"><ChevronRight :size="14" /><span class="folder">node_modules</span></div>
            <div class="vs-tree-row"><Braces :size="14" class="json-icon" /><span>package.json</span></div>
            <div class="vs-tree-row">
              <span class="file-indent"></span><span class="git-icon">◆</span><span>.gitignore</span>
            </div>
            <div class="vs-tree-heading lower"><ChevronRight :size="14" /><strong>OUTLINE</strong></div>
            <div class="vs-tree-heading"><ChevronRight :size="14" /><strong>TIMELINE</strong></div>
          </aside>

          <main class="vs-editor">
            <header class="vs-tabs">
              <div class="vs-tab active"><Braces :size="14" class="ts-icon" /><span>server.ts</span><i>●</i></div>
              <div class="vs-tab"><Braces :size="14" class="ts-icon" /><span>router.ts</span><X :size="13" /></div>
            </header>
            <div class="vs-breadcrumb">
              <span>src</span><ChevronRight :size="13" /><Braces :size="13" class="ts-icon" /><span>server.ts</span
              ><ChevronRight :size="13" /><span>server</span>
            </div>
            <div class="vs-code-scroll">
              <div class="vs-code">
                <div v-for="(line, index) in VSCODE_SAMPLE_LINES" :key="index" class="vs-code-line">
                  <span class="vs-line-number">{{ index + 1 }}</span>
                  <code
                    ><span
                      v-for="(token, tokenIndex) in line"
                      :key="tokenIndex"
                      :class="`tone-${token.tone || 'plain'}`"
                      >{{ token.text }}</span
                    ></code
                  >
                </div>
              </div>
              <div class="vs-minimap" aria-hidden="true">
                <i v-for="index in 17" :key="index" :style="{ width: `${34 + ((index * 23) % 54)}%` }"></i>
              </div>
            </div>
            <section class="vs-panel">
              <header>
                <strong>PROBLEMS</strong><span>OUTPUT</span><span>DEBUG CONSOLE</span
                ><span class="active">TERMINAL</span><b>⌄</b><X :size="14" />
              </header>
              <pre><span class="terminal-prompt">➜</span>  workspace <span class="terminal-branch">git:(feature/server-timing)</span> bun test

  24 pass
  0 fail
  68 expect() calls
  Ran 24 tests across 8 files. <span class="terminal-time">[412.00ms]</span>

<span class="terminal-prompt">➜</span>  workspace <span class="terminal-branch">git:(feature/server-timing)</span> <i></i></pre>
            </section>
          </main>
        </div>

        <footer class="vs-statusbar">
          <div>
            <span><GitBranch :size="13" /> feature/server-timing*</span><span>↻</span><span>ⓧ 0</span><span>△ 0</span>
          </div>
          <div>
            <span>Ln 18, Col 3</span><span>Spaces: 2</span><span>UTF-8</span><span>{ } TypeScript</span
            ><Bell :size="13" />
          </div>
        </footer>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped src="../vscodeDisguise.css"></style>
