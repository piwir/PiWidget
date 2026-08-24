<script setup>
import { ref } from 'vue';
import { useStore } from '../store/useStore';
import NoteCard from './NoteCard.vue';

const { state, expandedId, justCreatedId, addNote, moveNote } = useStore();

const dragFrom = ref(null);
const dragOver = ref(null);

function onDrop(target) {
  if (dragFrom.value !== null && dragFrom.value !== target) {
    moveNote(dragFrom.value, target);
  }
  dragFrom.value = null;
  dragOver.value = null;
}
</script>

<template>
  <div class="notes">
    <div v-if="state.notes.length === 0" class="notes-empty">
      <p class="notes-empty-title">还没有便笺</p>
      <p class="notes-empty-hint">点击下方 + 新建</p>
    </div>
    <NoteCard
      v-for="(n, i) in state.notes"
      :key="n.id"
      :note="n"
      :expanded="expandedId === n.id"
      :just-created="justCreatedId === n.id"
      :drag-over="dragOver === i && dragFrom !== null && dragFrom !== i"
      @toggle="expandedId = expandedId === n.id ? null : n.id"
      @dragstart="dragFrom = i"
      @dragover="dragOver = i"
      @drop="onDrop(i)"
      @dragend="dragFrom = null; dragOver = null"
    />
    <button class="note-add" @click="addNote">+ 新增便笺</button>
  </div>
</template>

<style scoped>
.notes {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 12px 18px 14px;
  gap: 8px;
  overflow-y: auto;
}

.notes-empty {
  flex: 1;
  display: grid;
  place-items: center;
  align-content: center;
  gap: 6px;
  border: 1px dashed var(--line);
  border-radius: 6px;
}
.notes-empty-title {
  margin: 0;
  font-size: 13px;
  color: var(--text);
  opacity: 0.75;
}
.notes-empty-hint {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.06em;
  color: var(--muted);
}

.note-add {
  flex: none;
  border: 1px dashed var(--line);
  background: transparent;
  color: var(--muted);
  font-family: var(--font-body);
  font-size: 13px;
  padding: 8px 0;
  border-radius: 6px;
  cursor: pointer;
  transition:
    color 150ms ease,
    border-color 150ms ease;
}
.note-add:hover {
  color: var(--text);
  border-color: var(--muted);
}
.note-add:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--highlight) 60%, transparent);
  outline-offset: -2px;
}
</style>
