<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useStore } from '../store/useStore';

const {
  state,
  activeTimetable,
  setCell,
  setActiveTimetable,
  addTimetable,
  renameTimetable,
  deleteTimetable,
  toggleTimetableCollapse,
} = useStore();

const DAYS = ['一', '二', '三', '四', '五', '六'];
const PERIODS = Array.from({ length: 8 }, (_, i) => i + 1);
const DAY_NUMS = [1, 2, 3, 4, 5, 6];

const today = computed(() => {
  const d = new Date().getDay();
  return d === 0 ? null : d;
});

const editing = ref(null); // { day, period }
const editText = ref('');

function cellVal(day, period) {
  const tt = activeTimetable.value;
  return (tt && (tt.cells[String(day)] || {})[String(period)]) || '';
}

// 重命名（inline input，Enter/blur 提交，Esc 取消）
const renaming = ref(false);
const renameText = ref('');
const renameInput = ref(null);

watch(renaming, (v) => {
  if (v)
    nextTick(() => {
      if (renameInput.value) {
        renameInput.value.focus();
        renameInput.value.select();
      }
    });
});

function beginRename() {
  renameText.value = activeTimetable.value ? activeTimetable.value.name : '';
  renaming.value = true;
}

function commitRename(save) {
  if (!renaming.value) return;
  renaming.value = false;
  if (save && activeTimetable.value) renameTimetable(activeTimetable.value.id, renameText.value);
}

function begin(day, period) {
  editText.value = cellVal(day, period);
  editing.value = { day, period };
}

function commit(save) {
  const e = editing.value;
  if (!e) return;
  if (save) setCell(e.day, e.period, editText.value.trim());
  editing.value = null;
}
</script>

<template>
  <section class="tt-panel">
    <div class="tt-header">
      <button
        class="tt-toggle"
        :aria-expanded="!state.timetableCollapsed"
        @click="toggleTimetableCollapse()"
      >
        <span>课表</span>
        <span class="tt-arrow">▾</span>
      </button>
      <div class="tt-switcher">
        <input
          v-if="renaming"
          ref="renameInput"
          v-model="renameText"
          class="tt-rename"
          aria-label="课表名称"
          @keyup.enter="commitRename(true)"
          @keyup.esc="commitRename(false)"
          @blur="commitRename(true)"
        />
        <select
          v-else
          class="tt-select"
          :value="state.activeTimetableId"
          title="切换课表"
          @change="setActiveTimetable($event.target.value)"
        >
          <option v-for="t in state.timetables" :key="t.id" :value="t.id">{{ t.name }}</option>
        </select>
        <button class="tt-mini" title="重命名当前课表" @click="beginRename">✎</button>
        <button class="tt-mini" title="新建课表" @click="addTimetable">＋</button>
        <button
          class="tt-mini"
          title="删除当前课表"
          :disabled="state.timetables.length <= 1"
          @click="deleteTimetable(state.activeTimetableId)"
        >
          ×
        </button>
      </div>
    </div>
    <div class="tt-grid-wrap">
      <div class="tt-grid">
        <div class="tt-cell tt-head">节</div>
        <div
          v-for="d in DAY_NUMS"
          :key="'h' + d"
          class="tt-cell tt-head"
          :class="{ today: today === d }"
        >
          {{ DAYS[d - 1] }}
        </div>
        <template v-for="p in PERIODS" :key="p">
          <div class="tt-cell tt-period">{{ p }}</div>
          <button
            v-for="d in DAY_NUMS"
            :key="d + '-' + p"
            class="tt-cell tt-slot"
            :class="{ today: today === d, empty: !cellVal(d, p) }"
            :title="cellVal(d, p) ? cellVal(d, p) + '（点击编辑）' : '点击添加课程'"
            @click="begin(d, p)"
          >
            <input
              v-if="editing && editing.day === d && editing.period === p"
              v-model="editText"
              class="tt-edit"
              :aria-label="`第${p}节 周${DAYS[d - 1]}课程`"
              @click.stop
              @keyup.enter="commit(true)"
              @keyup.esc="commit(false)"
              @blur="commit(true)"
            />
            <template v-else>{{ cellVal(d, p) || '—' }}</template>
          </button>
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.tt-panel {
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.tt-header {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
  padding: 0 10px 0 18px;
  border-bottom: 1px solid var(--line);
}
.tt-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: 1;
  border: none;
  background: transparent;
  color: var(--text);
  /* 区块标题与品牌同声音（display 窄体） */
  font-family: var(--font-display);
  font-size: 16px;
  font-weight: 700;
  letter-spacing: 0.06em;
  padding: 10px 0;
  cursor: pointer;
}

/* 课表切换器 */
.tt-switcher {
  display: flex;
  align-items: center;
  gap: 4px;
}
.tt-select {
  background: var(--card);
  color: var(--text);
  border: 1px solid var(--line);
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
  padding: 3px 6px;
  max-width: 120px;
  cursor: pointer;
}
.tt-rename {
  width: 120px;
  background: var(--card);
  color: var(--text);
  border: 1px solid var(--highlight);
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
  padding: 3px 6px;
  outline: none;
}
.tt-mini {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid var(--line);
  background: transparent;
  color: var(--muted);
  border-radius: 6px;
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
  transition:
    color 150ms ease,
    border-color 150ms ease;
}
.tt-mini:hover:not(:disabled) {
  color: var(--text);
  border-color: var(--muted);
}
.tt-mini:disabled {
  opacity: 0.35;
  cursor: default;
}
.tt-arrow {
  transition: transform 150ms ease;
  color: var(--muted);
}
.tt-toggle[aria-expanded='false'] .tt-arrow {
  transform: rotate(-90deg);
}

/* 收起/展开：0fr ↔ 1fr 高度过渡 */
.tt-grid-wrap {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-rows: 1fr;
  transition: grid-template-rows 150ms ease;
}
/* App 的 .sheet.collapsed 时收起（跨作用域；整个选择器必须在 :global() 内，
   否则 scoped 编译器会丢弃 :global() 之后的部分） */
:global(.sheet.collapsed .tt-grid-wrap) {
  grid-template-rows: 0fr;
}
:global(.sheet.collapsed .tt-header) {
  border-bottom: none;
}

.tt-grid {
  overflow: hidden;
  min-height: 0;
}

/* ============ 网格 ============ */
.tt-grid {
  display: grid;
  grid-template-columns: 38px repeat(6, minmax(0, 1fr));
  grid-auto-rows: 1fr;
  height: 100%;
  background: var(--line);
  gap: 1px;
}

.tt-cell {
  background: var(--ink);
  border: none;
  padding: 4px 7px;
  font-family: var(--font-body);
  font-size: 13px;
  line-height: 1.35;
  color: var(--text);
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
  cursor: default;
}

.tt-head {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.14em;
  color: var(--muted);
  text-align: center;
  padding: 5px 2px;
}

.tt-period {
  font-family: var(--font-mono);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: var(--muted);
  text-align: right;
  padding-right: 8px;
}

/* ============ 有课 / 无课层次 ============
   课程名 = 网格的声音（与品牌 / Hero 同字体：display 窄体，CJK 落回系统 CJK）
   有课：card 表面抬升；无课：墨底 + 小破折号，弱化 0.35
   今日列：荧光笔染色，今日+有课为全页最亮格 */
.tt-slot {
  cursor: text;
  text-align: center;
  font-family: var(--font-display);
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.02em;
  background: var(--card);
}
.tt-slot.empty {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 400;
  letter-spacing: 0;
  color: var(--muted);
  opacity: 0.35;
  background: var(--ink);
}
.tt-slot:hover {
  background: color-mix(in srgb, var(--text) 8%, var(--card));
}
.tt-slot.empty:hover {
  background: var(--card);
}

/* 今日列：淡荧光黄底 + 表头荧光笔下划线 */
.tt-slot.today,
.tt-head.today {
  background: color-mix(in srgb, var(--highlight) 8%, var(--ink));
}
.tt-slot.today:not(.empty) {
  background: color-mix(in srgb, var(--highlight) 14%, var(--card));
}
.tt-slot.today:hover {
  background: color-mix(in srgb, var(--highlight) 14%, var(--ink));
}
.tt-slot.today:not(.empty):hover {
  background: color-mix(in srgb, var(--highlight) 20%, var(--card));
}
.tt-head.today {
  color: var(--text);
  font-weight: 500;
  position: relative;
}
.tt-head.today::after {
  content: '';
  position: absolute;
  left: 20%;
  right: 20%;
  bottom: 2px;
  height: 2px;
  background: var(--highlight);
}

/* 单元格 inline 编辑（字体与课程名一致，透明底透出格子底色） */
.tt-edit {
  width: 100%;
  height: 100%;
  border: none;
  outline: 2px solid var(--highlight);
  outline-offset: -2px;
  background: transparent;
  color: var(--text);
  font-family: var(--font-display);
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-align: center;
  padding: 3px 6px;
  user-select: text;
}

/* 键盘焦点：与 .tt-edit 荧光描边同语言 */
.tt-slot:focus-visible,
.tt-mini:focus-visible,
.tt-toggle:focus-visible,
.tt-select:focus-visible,
.tt-rename:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--highlight) 60%, transparent);
  outline-offset: -2px;
}
</style>
