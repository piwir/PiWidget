<script setup>
import { useStore } from '../store/useStore';

const { state, togglePin, exportData, importData } = useStore();
</script>

<template>
  <header class="titlebar">
    <span class="brand" data-tauri-drag-region>PIWIDGET</span>
    <div class="actions">
      <button
        class="icon-btn"
        :class="{ active: state.pinned }"
        :aria-pressed="state.pinned"
        title="置顶"
        @click="togglePin"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M9 4h6l-1 7 3 3v2H7v-2l3-3-1-7z" />
          <line x1="12" y1="16" x2="12" y2="21" />
        </svg>
      </button>
      <button class="icon-btn" title="导出数据" @click="exportData">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3v12m0 0 4-4m-4 4-4-4M4 21h16" />
        </svg>
      </button>
      <button class="icon-btn" title="导入数据" @click="importData">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 15V3m0 0 4 4m-4-4L8 7M4 21h16" />
        </svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 44px;
  flex: none;
  padding: 0 10px 0 18px;
  border-bottom: 1px solid var(--line);
  cursor: grab;
}
.titlebar:active {
  cursor: grabbing;
}

.brand {
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 19px;
  letter-spacing: 0.14em;
  color: var(--text);
}

.actions {
  display: flex;
  gap: 4px;
}

.icon-btn {
  position: relative;
  z-index: 0;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  border-radius: 6px;
}
/* CTA 同款：荧光笔从左扫满 */
.icon-btn::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
  background: var(--highlight);
  transform: scaleX(0);
  transform-origin: left;
  transition: transform 0.25s ease;
}
.icon-btn:hover::before {
  transform: scaleX(1);
}
.icon-btn:hover {
  color: var(--ink);
}
.icon-btn.active {
  border-color: var(--highlight);
}
.icon-btn.active::before {
  transform: scaleX(1);
}
.icon-btn.active {
  color: var(--ink);
}
</style>
