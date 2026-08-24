<script setup>
import { computed, onMounted, onBeforeUnmount, ref } from 'vue';
import { useStore } from '../store/useStore';

const { activeTimetable } = useStore();

const WEEK_EN = ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT'];
const WEEK_CN = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
const pad2 = (n) => String(n).padStart(2, '0');

const now = ref(new Date());
let timer = null;
onMounted(() => {
  timer = setInterval(() => (now.value = new Date()), 30000);
});
onBeforeUnmount(() => clearInterval(timer));

const eyebrow = computed(
  () => `${WEEK_EN[now.value.getDay()]} · ${pad2(now.value.getMonth() + 1)}/${pad2(now.value.getDate())}`
);
const dayCn = computed(() => WEEK_CN[now.value.getDay()]);
const dayCount = computed(() => {
  const d = now.value.getDay();
  if (d === 0) return 0;
  const t = (activeTimetable.value && activeTimetable.value.cells[String(d)]) || {};
  return Object.values(t).filter(Boolean).length;
});
const meta = computed(() =>
  dayCount.value > 0 ? `今天 ${dayCount.value} 节` : '今日无课 · 好好休息'
);
</script>

<template>
  <section class="hero">
    <div class="eyebrow">{{ eyebrow }}</div>
    <div class="hero-main">
      <span class="hero-day" :key="dayCn">{{ dayCn }}</span>
    </div>
    <div class="meta">{{ meta }}</div>
  </section>
</template>

<style scoped>
.hero {
  flex: none;
  text-align: center;
  padding: 22px 18px 14px;
  border-bottom: 1px solid var(--line);
}

.eyebrow {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.18em;
  color: var(--muted);
}

.hero-main {
  display: flex;
  align-items: baseline;
  justify-content: center;
  margin: 2px 0 6px;
  min-height: 1em;
}

.hero-day {
  position: relative;
  display: inline-block;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: clamp(56px, 8.6vw, 92px);
  line-height: 0.9;
  letter-spacing: 0.04em;
  /* 下方 0.16em 为荧光条净空区：条形完全落在字形之外，任何平台渲染零重叠 */
  padding: 0 0.06em 0.16em;
  color: var(--text);
}

/* 签名：荧光笔从左侧扫过（全页唯一编排动效） */
.hero-day::before {
  content: '';
  position: absolute;
  left: -0.05em;
  right: -0.05em;
  bottom: 0;
  height: 0.13em;
  background: var(--highlight);
  transform-origin: left center;
  animation: highlight-sweep 0.6s ease-out 0.15s both;
}

@keyframes highlight-sweep {
  from {
    transform: scaleX(0);
  }
  to {
    transform: scaleX(1);
  }
}

/* 减少动态偏好：黄条静态全显，跳过扫入 */
@media (prefers-reduced-motion: reduce) {
  .hero-day::before {
    animation: none;
  }
}

.meta {
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
