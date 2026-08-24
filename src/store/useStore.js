/* PiWidget · 全局 store（模块级单例）
 * 数据持久化：Tauri 下写 app_data_dir()/data.json（目录由 identifier 派生），浏览器下 localStorage。
 * schema v4：notes[] + timetables[{id,name,cells{day:{period:课程名}}}] + activeTimetableId
 *        + windowPos{x, y} / windowSize{w, h}（物理像素：窗口位置 / 展开态尺寸，导入时清空）。
 * v1（note 字符串）/ v2（单一 timetable）/ v3 加载/导入时自动迁移。
 */
import { reactive, ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { open, save } from '@tauri-apps/plugin-dialog';
import { useToast } from './useToast';

const { show: toast } = useToast();

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
const LS_KEY = 'piwidget-data';
const SAVE_DEBOUNCE = 800;

export function defaultState() {
  return {
    version: 4,
    pinned: false,
    ghost: false,
    timetableCollapsed: false,
    notes: [],
    timetables: [{ id: 'default', name: '默认', cells: {} }],
    activeTimetableId: 'default',
    windowPos: null, // { x, y } 物理像素（outerPosition 同单位）
    windowSize: null, // { w, h } 物理像素（展开态尺寸，outerSize 同单位）
  };
}

function cleanTimetable(t) {
  const out = {};
  if (typeof t !== 'object' || t === null) return out;
  for (const day of Object.keys(t)) {
    if (!/^[1-6]$/.test(day) || typeof t[day] !== 'object' || t[day] === null) continue;
    out[day] = {};
    for (const p of Object.keys(t[day])) {
      if (/^[1-8]$/.test(p) && typeof t[day][p] === 'string') out[day][p] = t[day][p];
    }
  }
  return out;
}

function cleanTimetables(tts) {
  const out = (Array.isArray(tts) ? tts : [])
    .filter((t) => t && typeof t === 'object')
    .map((t, i) => ({
      id: typeof t.id === 'string' && t.id ? t.id : `tt-${i}`,
      name: typeof t.name === 'string' && t.name ? t.name : `课表 ${i + 1}`,
      cells: cleanTimetable(t.cells),
    }));
  return out.length ? out : [{ id: 'default', name: '默认', cells: {} }];
}

function cleanNotes(notes) {
  if (!Array.isArray(notes)) return [];
  return notes
    .filter((n) => n && typeof n === 'object')
    .map((n) => ({
      id: typeof n.id === 'string' && n.id ? n.id : crypto.randomUUID(),
      title: typeof n.title === 'string' ? n.title : '',
      content: typeof n.content === 'string' ? n.content : '',
    }));
}

/** 归一化（v1 / v2 / v3 / v4 均接受），返回 v4 state */
export function migrate(data) {
  const d = defaultState();
  if (typeof data !== 'object' || data === null) return d;
  if (typeof data.pinned === 'boolean') d.pinned = data.pinned;
  if (typeof data.ghost === 'boolean') d.ghost = data.ghost;
  if (typeof data.timetableCollapsed === 'boolean') d.timetableCollapsed = data.timetableCollapsed;
  if (Array.isArray(data.timetables)) {
    // v3：多课表
    d.timetables = cleanTimetables(data.timetables);
    d.activeTimetableId =
      typeof data.activeTimetableId === 'string' &&
      d.timetables.some((t) => t.id === data.activeTimetableId)
        ? data.activeTimetableId
        : d.timetables[0].id;
  } else {
    // v1 / v2：单一 timetable → 包进「默认」
    d.timetables = [{ id: 'default', name: '默认', cells: cleanTimetable(data.timetable) }];
    d.activeTimetableId = 'default';
  }
  if (Array.isArray(data.notes)) {
    d.notes = cleanNotes(data.notes);
  } else if (typeof data.note === 'string' && data.note) {
    // v1 迁移：单条 note 字符串 → 单条便笺
    d.notes = [{ id: crypto.randomUUID(), title: '便笺', content: data.note }];
  }
  // v4：窗口位置 / 展开态尺寸（物理像素）；v1–v3 数据自然得 null
  if (
    data.windowPos &&
    typeof data.windowPos === 'object' &&
    Number.isFinite(data.windowPos.x) &&
    Number.isFinite(data.windowPos.y)
  ) {
    d.windowPos = { x: data.windowPos.x, y: data.windowPos.y };
  }
  if (
    data.windowSize &&
    typeof data.windowSize === 'object' &&
    Number.isFinite(data.windowSize.w) &&
    Number.isFinite(data.windowSize.h) &&
    data.windowSize.w > 0 &&
    data.windowSize.h > 0
  ) {
    d.windowSize = { w: data.windowSize.w, h: data.windowSize.h };
  }
  return d;
}

/** 导入校验：返回错误描述或 null */
export function validate(data) {
  if (typeof data !== 'object' || data === null) return '不是有效的对象';
  if (data.version === 1) {
    if (typeof data.note !== 'string') return 'note 不是字符串';
    if (typeof data.timetable !== 'object' || data.timetable === null) return 'timetable 不是对象';
    return null;
  }
  if (data.version === 2) {
    if (!Array.isArray(data.notes)) return 'notes 不是数组';
    for (const n of data.notes) {
      if (!n || typeof n !== 'object' || typeof n.title !== 'string' || typeof n.content !== 'string') {
        return 'notes 元素需含字符串 title/content';
      }
    }
    if (typeof data.timetable !== 'object' || data.timetable === null) return 'timetable 不是对象';
    return null;
  }
  if (data.version === 3) {
    if (!Array.isArray(data.notes)) return 'notes 不是数组';
    for (const n of data.notes) {
      if (!n || typeof n !== 'object' || typeof n.title !== 'string' || typeof n.content !== 'string') {
        return 'notes 元素需含字符串 title/content';
      }
    }
    if (!Array.isArray(data.timetables)) return 'timetables 不是数组';
    return null;
  }
  if (data.version === 4) {
    if (!Array.isArray(data.notes)) return 'notes 不是数组';
    for (const n of data.notes) {
      if (!n || typeof n !== 'object' || typeof n.title !== 'string' || typeof n.content !== 'string') {
        return 'notes 元素需含字符串 title/content';
      }
    }
    if (!Array.isArray(data.timetables)) return 'timetables 不是数组';
    if (
      data.windowPos != null &&
      (typeof data.windowPos !== 'object' ||
        !Number.isFinite(data.windowPos.x) ||
        !Number.isFinite(data.windowPos.y))
    ) {
      return 'windowPos 需为 null 或 {x, y} 数字';
    }
    if (
      data.windowSize != null &&
      (typeof data.windowSize !== 'object' ||
        !Number.isFinite(data.windowSize.w) ||
        !Number.isFinite(data.windowSize.h))
    ) {
      return 'windowSize 需为 null 或 {w, h} 数字';
    }
    return null;
  }
  return `不支持的 version（当前 ${JSON.stringify(data.version)}）`;
}

// ---------- 单例状态 ----------
const state = reactive(defaultState());
const expandedId = ref(null); // 当前展开的便笺 id
const justCreatedId = ref(null); // 新建后待聚焦的便笺 id
const activeTimetable = computed(
  () => state.timetables.find((t) => t.id === state.activeTimetableId) || state.timetables[0]
);

let dataPath = '';
let saveTimer = null;

async function persist(raw) {
  if (isTauri) {
    if (!dataPath) dataPath = await invoke('data_path');
    await writeTextFile(dataPath, raw);
  } else {
    localStorage.setItem(LS_KEY, raw);
  }
}

async function saveNow() {
  try {
    await persist(JSON.stringify(state, null, 2));
  } catch (e) {
    toast('保存失败：' + e, true);
  }
}

function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, SAVE_DEBOUNCE);
}

async function load() {
  let raw = null;
  if (isTauri) {
    dataPath = await invoke('data_path');
    try {
      raw = await readTextFile(dataPath);
    } catch {
      raw = null;
    }
  } else {
    raw = localStorage.getItem(LS_KEY);
  }

  if (raw === null || raw === '') {
    Object.assign(state, defaultState());
    await saveNow();
    return;
  }
  try {
    const parsed = JSON.parse(raw);
    if (
      typeof parsed !== 'object' ||
      parsed === null ||
      (parsed.version !== 1 && parsed.version !== 2 && parsed.version !== 3 &&
        parsed.version !== 4)
    ) {
      throw new Error('bad schema');
    }
    Object.assign(state, migrate(parsed));
  } catch {
    // 备份 + 恢复默认
    if (isTauri) {
      try {
        await writeTextFile(dataPath + '.bak', raw);
      } catch {
        /* 备份失败不影响恢复 */
      }
    } else {
      try {
        localStorage.setItem(LS_KEY + '.bak', raw);
      } catch {
        /* 同上 */
      }
    }
    Object.assign(state, defaultState());
    await saveNow();
    toast('数据文件损坏，已恢复默认（原数据备份为 .bak）', true);
  }
}

// ---------- 便笺 ----------
function addNote() {
  const note = { id: crypto.randomUUID(), title: '', content: '' };
  state.notes.push(note);
  expandedId.value = note.id;
  justCreatedId.value = note.id;
  saveSoon();
  return note.id;
}

function deleteNote(id) {
  const i = state.notes.findIndex((n) => n.id === id);
  if (i < 0) return;
  state.notes.splice(i, 1);
  if (expandedId.value === id) expandedId.value = null;
  saveSoon();
  toast('已删除');
}

function moveNote(from, to) {
  if (from === to || from < 0 || to < 0 || from >= state.notes.length || to >= state.notes.length) return;
  const [n] = state.notes.splice(from, 1);
  state.notes.splice(to, 0, n);
  saveSoon();
}

// ---------- 课表（操作当前激活课表） ----------
function setCell(day, period, text) {
  const tt = activeTimetable.value;
  if (!tt) return;
  const key = String(period);
  const cells = { ...tt.cells };
  const d = { ...(cells[String(day)] || {}) };
  if (text) d[key] = text;
  else delete d[key];
  if (Object.keys(d).length > 0) cells[String(day)] = d;
  else delete cells[String(day)];
  tt.cells = cells;
  saveSoon();
}

function setActiveTimetable(id) {
  if (state.timetables.some((t) => t.id === id)) {
    state.activeTimetableId = id;
    saveSoon();
  }
}

function addTimetable() {
  const tt = { id: crypto.randomUUID(), name: `课表 ${state.timetables.length + 1}`, cells: {} };
  state.timetables.push(tt);
  state.activeTimetableId = tt.id;
  saveSoon();
  toast(`已新建「${tt.name}」`);
}

function renameTimetable(id, name) {
  const tt = state.timetables.find((t) => t.id === id);
  if (!tt) return;
  const trimmed = String(name || '').trim();
  if (!trimmed) {
    toast('名称不能为空', true);
    return;
  }
  tt.name = trimmed;
  saveSoon();
  toast(`已重命名为「${trimmed}」`);
}

function deleteTimetable(id) {
  if (state.timetables.length <= 1) {
    toast('至少保留一个课表', true);
    return;
  }
  const i = state.timetables.findIndex((t) => t.id === id);
  if (i < 0) return;
  const [removed] = state.timetables.splice(i, 1);
  if (state.activeTimetableId === id) state.activeTimetableId = state.timetables[0].id;
  saveSoon();
  toast(`已删除「${removed.name}」`);
}

// ---------- 透明化（卡片淡出，桌面透出） ----------
function toggleGhost() {
  state.ghost = !state.ghost;
  saveSoon();
  toast(state.ghost ? '已透明化' : '已恢复显示');
}

// ---------- 置顶（仅 Tauri；默认不置顶，启动时同步已保存状态） ----------
async function syncPinned() {
  if (!isTauri) return;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().setAlwaysOnTop(state.pinned);
  } catch {
    /* 忽略 */
  }
}

async function togglePin() {
  state.pinned = !state.pinned;
  saveSoon();
  toast(state.pinned ? '已置顶' : '已取消置顶');
  await syncPinned();
}

// ---------- 窗口位置 / 展开尺寸（仅 Tauri；物理像素；导入时清空） ----------

/**
 * 启动时恢复窗口尺寸 + 位置，并注册移动/缩放监听（持久化走现有 deep watch → saveSoon）。
 * 位置仅在与任一屏幕相交时恢复（防拔显示器 / 分辨率变化后落到屏外）。
 */
async function restoreWindowState() {
  if (!isTauri) return;
  try {
    const { getCurrentWindow, availableMonitors } = await import('@tauri-apps/api/window');
    const { PhysicalPosition, PhysicalSize } = await import('@tauri-apps/api/dpi');
    const win = getCurrentWindow();
    const size = state.windowSize;
    const pos = state.windowPos;
    if (size && size.w > 0 && size.h > 0) {
      await win.setSize(new PhysicalSize(size.w, size.h));
    }
    if (pos && Number.isFinite(pos.x) && Number.isFinite(pos.y)) {
      const scale = await win.scaleFactor();
      const w = size?.w ?? Math.round(760 * scale);
      const h = size?.h ?? Math.round(540 * scale);
      const monitors = await availableMonitors();
      const onScreen = monitors.some(
        (m) =>
          pos.x < m.position.x + m.size.width &&
          pos.x + w > m.position.x &&
          pos.y < m.position.y + m.size.height &&
          pos.y + h > m.position.y
      );
      if (onScreen) await win.setPosition(new PhysicalPosition(pos.x, pos.y));
    }
    win.onMoved(({ payload }) => {
      state.windowPos = { x: payload.x, y: payload.y };
    });
    win.onResized(({ payload }) => {
      // 收起态手动拖宽不污染展开尺寸（高度跟随）
      state.windowSize = state.timetableCollapsed
        ? { w: state.windowSize?.w ?? payload.w, h: payload.h }
        : { w: payload.w, h: payload.h };
    });
  } catch {
    /* 忽略：窗口状态恢复失败不影响主流程 */
  }
}

/**
 * 课表收起/展开后把窗口宽度同步到对应状态（仅 Tauri；高度不变）。
 * 收起目标宽 = (当前展开宽 − 2px sheet 边框) × 0.42 + 2，
 * 0.42 对应 App.vue 的 42fr/58fr 双栏网格 —— 改网格比例须同步此系数。
 * 收起态临时放宽 minWidth（560 → 200：最小展开宽 560 → 收起目标 ≈ 236）。
 */
async function applyTimetableResize() {
  if (!isTauri) return;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const { PhysicalSize } = await import('@tauri-apps/api/dpi');
    const win = getCurrentWindow();
    const scale = await win.scaleFactor();
    const cur = await win.outerSize(); // 物理像素；收起分支调用时窗口仍是展开宽
    if (state.timetableCollapsed) {
      const targetW = Math.round((cur.width / scale - 2) * 0.42 + 2) * scale;
      const minW = Math.round(200 * scale);
      await win.setMinSize(new PhysicalSize(minW, Math.round(360 * scale)));
      await win.setSize(new PhysicalSize(Math.max(targetW, minW), cur.height));
    } else {
      const s = state.windowSize;
      const w = s?.w ?? Math.round(760 * scale);
      const h = s?.h ?? Math.round(540 * scale);
      await win.setSize(new PhysicalSize(w, h));
      await win.setMinSize(new PhysicalSize(Math.round(560 * scale), Math.round(360 * scale)));
    }
  } catch {
    /* 忽略 */
  }
}

async function toggleTimetableCollapse() {
  state.timetableCollapsed = !state.timetableCollapsed;
  saveSoon();
  await applyTimetableResize();
}

// ---------- 开机自启（仅 Tauri；OS 注册表是事实源，不写入数据文件） ----------
const autostart = ref(false);

async function refreshAutostart() {
  if (!isTauri) return;
  try {
    const { isEnabled } = await import('@tauri-apps/plugin-autostart');
    autostart.value = await isEnabled();
  } catch {
    /* 忽略 */
  }
}

async function toggleAutostart() {
  if (!isTauri) {
    toast('开机自启仅桌面端可用', true);
    return;
  }
  try {
    const { enable, disable } = await import('@tauri-apps/plugin-autostart');
    if (autostart.value) {
      await disable();
      autostart.value = false;
      toast('已关闭开机自启');
    } else {
      await enable();
      autostart.value = true;
      toast('已开启开机自启');
    }
  } catch (e) {
    toast('自启设置失败：' + e, true);
  }
}

// ---------- 导入 / 导出 ----------
async function exportData() {
  const json = JSON.stringify(state, null, 2);
  try {
    if (isTauri) {
      const p = await save({
        defaultPath: 'piwidget-data.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!p) return;
      await invoke('write_file', { path: p, content: json });
      toast('已导出 ' + String(p).split(/[\\/]/).pop());
    } else {
      const blob = new Blob([json], { type: 'application/json' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = 'piwidget-data.json';
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 5000);
      toast('已导出 piwidget-data.json');
    }
  } catch (e) {
    toast('导出失败：' + e, true);
  }
}

async function importRaw(raw) {
  let data;
  try {
    data = JSON.parse(raw);
  } catch {
    toast('导入失败：不是有效的 JSON', true);
    return;
  }
  const err = validate(data);
  if (err) {
    toast('导入失败：' + err, true);
    return;
  }
  Object.assign(state, migrate(data));
  // 窗口坐标是机器相关的：导入他人数据不应让窗口跳位
  state.windowPos = null;
  state.windowSize = null;
  expandedId.value = null;
  justCreatedId.value = null;
  await saveNow();
  toast('已导入');
}

async function importData() {
  try {
    if (isTauri) {
      const p = await open({
        multiple: false,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!p) return;
      await importRaw(await invoke('read_file', { path: p }));
    } else {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json,application/json';
      input.onchange = async () => {
        const file = input.files && input.files[0];
        if (file) await importRaw(await file.text());
      };
      input.click();
    }
  } catch (e) {
    toast('导入失败：' + e, true);
  }
}

export function useStore() {
  return {
    state,
    expandedId,
    justCreatedId,
    activeTimetable,
    autostart,
    refreshAutostart,
    toggleAutostart,
    load,
    saveNow,
    saveSoon,
    addNote,
    deleteNote,
    moveNote,
    setCell,
    setActiveTimetable,
    addTimetable,
    renameTimetable,
    deleteTimetable,
    togglePin,
    syncPinned,
    restoreWindowState,
    applyTimetableResize,
    toggleTimetableCollapse,
    toggleGhost,
    exportData,
    importData,
  };
}
