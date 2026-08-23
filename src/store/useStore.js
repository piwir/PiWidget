/* PiWidget · 全局 store（模块级单例）
 * 数据持久化：Tauri 下写 app_data_dir()/data.json（目录由 identifier 派生），浏览器下 localStorage。
 * schema v2：notes[] + timetable{day: {period: 课程名}}，无时间逻辑。
 * v1（note 字符串 + periods）加载/导入时自动迁移。
 */
import { reactive, ref } from 'vue';
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
    version: 2,
    pinned: true,
    timetableCollapsed: false,
    notes: [],
    timetable: {},
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

/** 归一化（v1 / v2 均接受），返回 v2 state */
export function migrate(data) {
  const d = defaultState();
  if (typeof data !== 'object' || data === null) return d;
  if (typeof data.pinned === 'boolean') d.pinned = data.pinned;
  if (typeof data.timetableCollapsed === 'boolean') d.timetableCollapsed = data.timetableCollapsed;
  d.timetable = cleanTimetable(data.timetable);
  if (Array.isArray(data.notes)) {
    d.notes = cleanNotes(data.notes);
  } else if (typeof data.note === 'string' && data.note) {
    // v1 迁移：单条 note 字符串 → 单条便笺
    d.notes = [{ id: crypto.randomUUID(), title: '便笺', content: data.note }];
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
  if (data.version !== 2) return `不支持的 version（当前 ${JSON.stringify(data.version)}）`;
  if (!Array.isArray(data.notes)) return 'notes 不是数组';
  for (const n of data.notes) {
    if (!n || typeof n !== 'object' || typeof n.title !== 'string' || typeof n.content !== 'string') {
      return 'notes 元素需含字符串 title/content';
    }
  }
  if (typeof data.timetable !== 'object' || data.timetable === null) return 'timetable 不是对象';
  return null;
}

// ---------- 单例状态 ----------
const state = reactive(defaultState());
const expandedId = ref(null); // 当前展开的便笺 id
const justCreatedId = ref(null); // 新建后待聚焦的便笺 id

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
    if (typeof parsed !== 'object' || parsed === null || (parsed.version !== 1 && parsed.version !== 2)) {
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

// ---------- 课表 ----------
function setCell(day, period, text) {
  const key = String(period);
  if (text) {
    state.timetable[String(day)] = { ...(state.timetable[String(day)] || {}), [key]: text };
  } else {
    const d = { ...(state.timetable[String(day)] || {}) };
    delete d[key];
    if (Object.keys(d).length > 0) state.timetable[String(day)] = d;
    else delete state.timetable[String(day)];
  }
  saveSoon();
}

// ---------- 置顶（仅 Tauri） ----------
async function togglePin() {
  state.pinned = !state.pinned;
  if (isTauri) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().setAlwaysOnTop(state.pinned);
    } catch {
      /* 忽略 */
    }
  }
  saveSoon();
  toast(state.pinned ? '已置顶' : '已取消置顶');
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
    load,
    saveNow,
    saveSoon,
    addNote,
    deleteNote,
    moveNote,
    setCell,
    togglePin,
    exportData,
    importData,
  };
}
