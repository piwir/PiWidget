<script setup>
import { onMounted, onBeforeUnmount, watch } from 'vue';
import TitleBar from './components/TitleBar.vue';
import Hero from './components/Hero.vue';
import NoteList from './components/NoteList.vue';
import Timetable from './components/Timetable.vue';
import { useStore } from './store/useStore';
import { useToast } from './store/useToast';

const { state, saveSoon, saveNow, load } = useStore();
const { toast, show } = useToast();

// 任何状态变化 → 防抖落盘
watch(state, saveSoon, { deep: true });

function onKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    saveNow().then(() => show('已保存'));
  }
}

onMounted(() => {
  load();
  window.addEventListener('keydown', onKeydown);
});
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown));
</script>

<template>
  <div class="sheet" :class="{ collapsed: state.timetableCollapsed }">
    <TitleBar />
    <div class="body">
      <div class="left">
        <Hero />
        <NoteList />
      </div>
      <Timetable />
    </div>
    <div class="toast" :class="{ show: toast.visible, error: toast.error }">{{ toast.message }}</div>
  </div>
</template>

<style scoped>
/* ============ 墨色卡片 ============ */
.sheet {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--ink);
  border: 1px solid color-mix(in srgb, var(--text) 18%, transparent);
  border-radius: var(--radius);
  overflow: hidden;
}

/* ============ 主体双栏 ============ */
.body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 42fr 58fr;
  grid-template-rows: minmax(0, 1fr) auto;
}

.left {
  grid-row: 1;
  grid-column: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--line);
}

:deep(.tt-panel) {
  grid-row: 1 / 3;
  grid-column: 2;
}

/* 收起：左栏占满整行，课表标题条落到最下方 */
.sheet.collapsed .body {
  grid-template-columns: 1fr;
}
.sheet.collapsed .left {
  grid-column: 1;
  border-right: none;
}
.sheet.collapsed :deep(.tt-panel) {
  grid-row: 2;
  grid-column: 1;
}

/* ============ Toast ============ */
.toast {
  position: fixed;
  left: 50%;
  bottom: 16px;
  transform: translateX(-50%) translateY(4px);
  background: var(--card);
  border: 1px solid color-mix(in srgb, var(--text) 25%, transparent);
  border-radius: 6px;
  padding: 6px 14px;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text);
  opacity: 0;
  pointer-events: none;
  transition:
    opacity 150ms ease,
    transform 150ms ease;
  max-width: 80%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.toast.show {
  opacity: 1;
  transform: translateX(-50%) translateY(0);
}
.toast.error {
  background: var(--highlight);
  color: var(--ink);
  border-color: var(--highlight);
}
</style>
