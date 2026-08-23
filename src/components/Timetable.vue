<script setup>
import { computed, ref } from 'vue';
import { useStore } from '../store/useStore';

const { state, setCell } = useStore();

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
  return (state.timetable[String(day)] || {})[String(period)] || '';
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
    <button
      class="tt-toggle"
      :aria-expanded="!state.timetableCollapsed"
      @click="state.timetableCollapsed = !state.timetableCollapsed"
    >
      <span>课表</span>
      <span class="tt-arrow">▾</span>
    </button>
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

.tt-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  flex: none;
  border: none;
  border-bottom: 1px solid var(--line);
  background: transparent;
  color: var(--text);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 600;
  padding: 10px 18px;
  cursor: pointer;
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
/* App 的 .sheet.collapsed 时收起（跨作用域，用 :global 穿透） */
:global(.sheet.collapsed) .tt-grid-wrap {
  grid-template-rows: 0fr;
}
:global(.sheet.collapsed) .tt-toggle {
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
  font-size: 12px;
  letter-spacing: 0.08em;
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

.tt-slot {
  cursor: text;
}
.tt-slot:hover {
  background: var(--card);
}
.tt-slot.empty {
  color: var(--muted);
  opacity: 0.35;
}

/* 今日列：淡荧光黄底 + 表头荧光笔下划线 */
.tt-slot.today,
.tt-head.today {
  background: color-mix(in srgb, var(--highlight) 7%, var(--ink));
}
.tt-slot.today:hover {
  background: color-mix(in srgb, var(--highlight) 14%, var(--ink));
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

/* 单元格 inline 编辑 */
.tt-edit {
  width: 100%;
  height: 100%;
  border: none;
  outline: 2px solid var(--highlight);
  outline-offset: -2px;
  background: var(--ink);
  color: var(--text);
  font-family: var(--font-body);
  font-size: 13px;
  padding: 3px 6px;
  user-select: text;
}
</style>
