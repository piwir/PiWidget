<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useStore } from '../store/useStore';

const props = defineProps({
  note: { type: Object, required: true },
  expanded: { type: Boolean, default: false },
  justCreated: { type: Boolean, default: false },
  dragOver: { type: Boolean, default: false },
});
const emit = defineEmits(['toggle', 'dragstart', 'dragover', 'drop', 'dragend']);

const { deleteNote } = useStore();

const titleInput = ref(null);

const excerpt = computed(() => {
  const line = (props.note.content || '').split('\n').find((l) => l.trim());
  return line ? line.trim() : '';
});

// 新建的便笺：展开后自动聚焦标题
watch(
  () => props.expanded && props.justCreated,
  (v) => {
    if (v) nextTick(() => titleInput.value && titleInput.value.focus());
  },
  { immediate: true }
);
</script>

<template>
  <div
    class="note-card"
    :class="{ expanded, 'drag-over': dragOver }"
    :draggable="!expanded"
    @click="emit('toggle')"
    @dragstart="$event.dataTransfer.effectAllowed = 'move'; emit('dragstart')"
    @dragover.prevent="$event.dataTransfer.dropEffect = 'move'; emit('dragover')"
    @drop.prevent="emit('drop')"
    @dragend="emit('dragend')"
  >
    <template v-if="!expanded">
      <div class="note-title" :class="{ empty: !note.title }">{{ note.title || '未命名便笺' }}</div>
      <div class="note-excerpt" :class="{ empty: !excerpt }">{{ excerpt || '暂无内容' }}</div>
    </template>
    <template v-else>
      <input
        ref="titleInput"
        v-model="note.title"
        class="note-title-input"
        placeholder="标题"
        @click.stop
        @keyup.enter="emit('toggle')"
        @keyup.esc="emit('toggle')"
      />
      <textarea
        v-model="note.content"
        class="note-content"
        placeholder="写点什么…"
        @click.stop
      ></textarea>
    </template>
    <button class="note-del" title="删除便笺" @click.stop="deleteNote(note.id)">×</button>
  </div>
</template>

<style scoped>
.note-card {
  position: relative;
  flex: none;
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 8px 30px 8px 12px;
  cursor: pointer;
  /* 收起态可拖拽：grab 光标 + 防选中；展开态恢复默认 */
  &:has(.note-title) {
    cursor: grab;
    user-select: none;
  }
  &:has(.note-title):active {
    cursor: grabbing;
  }
  transition:
    background 150ms ease,
    border-color 150ms ease;
}
.note-card:hover {
  background: color-mix(in srgb, var(--card) 60%, transparent);
}
.note-card.expanded {
  cursor: default;
  background: var(--card);
  border-color: color-mix(in srgb, var(--text) 20%, transparent);
}
.note-card.drag-over {
  border-color: var(--highlight);
}

.note-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.note-title.empty {
  color: var(--muted);
  font-weight: 400;
}

.note-excerpt {
  font-size: 12px;
  color: var(--muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.note-excerpt.empty {
  opacity: 0.35;
}

.note-title-input {
  width: 100%;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 600;
  padding: 2px 0;
  user-select: text;
}
.note-title-input::placeholder {
  color: var(--muted);
  opacity: 0.35;
}

.note-content {
  width: 100%;
  min-height: 120px;
  max-height: 320px;
  border: none;
  outline: none;
  resize: vertical;
  background: transparent;
  color: var(--text);
  font-family: var(--font-body);
  font-size: 13px;
  line-height: 1.7;
  padding: 4px 0 2px;
  user-select: text;
}
.note-content::placeholder {
  color: var(--muted);
  opacity: 0.35;
}

.note-del {
  position: absolute;
  top: 5px;
  right: 5px;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--muted);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  opacity: 0;
  transition:
    opacity 150ms ease,
    background 150ms ease,
    color 150ms ease;
}
.note-card:hover .note-del,
.note-card.expanded .note-del {
  opacity: 1;
}
.note-del:hover {
  background: var(--highlight);
  color: var(--ink);
}
.note-del:focus-visible {
  opacity: 1;
  outline: 2px solid color-mix(in srgb, var(--highlight) 60%, transparent);
  outline-offset: -2px;
}
</style>
