#!/usr/bin/env python3
"""PiWidget 回归自检（25 项 + 黄条零重叠断言 ×3）+ README 截图。

用法: python3 scripts/regression.py [url]
默认 url = http://localhost:4173（vite preview）。
"""
import json
import sys
from datetime import date
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:4173"
DEMO_BG = "#23262E"  # 中性桌面色，展示透明圆角

WEEK_CN = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"]

passed, failed = [], []


def check(name, ok, detail=""):
    (passed if ok else failed).append(name)
    print(f"  {'✅' if ok else '❌'} {name}" + (f"  [{detail}]" if detail and not ok else ""))


def main():
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 980, "height": 720})
        page.goto(URL, wait_until="networkidle")
        page.wait_for_selector(".sheet")

        today = date.today()
        day_cn = WEEK_CN[(today.weekday() + 1) % 7]  # python Mon=0 → JS getDay Sun=0

        print("== 基础渲染 ==")
        check("01 页面加载 .sheet 存在", page.locator(".sheet").count() == 1)
        check("02 品牌字 PIWIDGET", page.locator(".titlebar .brand").inner_text() == "PIWIDGET")
        check("03 Hero 显示今日星期", page.locator(".hero-day").inner_text() == day_cn,
              f"expect {day_cn} got {page.locator('.hero-day').inner_text()}")
        eyebrow = page.locator(".eyebrow").inner_text()
        check("04 eyebrow 含 MM/DD", f"{today.month:02d}/{today.day:02d}" in eyebrow, eyebrow)

        print("== 黄条零重叠断言（核心） ==")
        bar = page.evaluate("""() => {
            const el = document.querySelector('.hero-day');
            const cs = getComputedStyle(el);
            const before = getComputedStyle(el, '::before');
            const r = el.getBoundingClientRect();
            const pb = parseFloat(cs.paddingBottom);
            const bh = parseFloat(before.height);
            const bottom = parseFloat(before.bottom);
            return {
                barTop: r.bottom - bottom - bh,   // bottom:0 → 条顶 = 元素底 - 条高
                contentBottom: r.bottom - pb,     // 内容盒底（字形净空区上界）
                barHeight: bh,
                clearance: pb,
                barBg: before.backgroundColor,
            };
        }""")
        check("H1 荧光条存在且为签名黄 #FFD84D", bar["barHeight"] > 0 and "255, 216, 77" in bar["barBg"],
              json.dumps(bar, ensure_ascii=False))
        check("H2 黄条顶部 ≥ 内容盒底（与字形零重叠）", bar["barTop"] >= bar["contentBottom"] - 0.5,
              json.dumps(bar, ensure_ascii=False))
        check("H3 净空区 ≥ 条高（0.16em ≥ 0.13em）", bar["clearance"] >= bar["barHeight"] - 0.01,
              json.dumps(bar, ensure_ascii=False))

        print("== 空态 ==")
        empty = page.locator(".notes-empty")
        check("05 空态虚线邀请框", empty.count() == 1 and
              page.evaluate("getComputedStyle(document.querySelector('.notes-empty')).borderStyle") == "dashed")
        check("06 空态主文案「还没有便笺」", empty.locator(".notes-empty-title").inner_text() == "还没有便笺")
        check("07 空态副文案「点击下方 + 新建」", empty.locator(".notes-empty-hint").inner_text() == "点击下方 + 新建")

        print("== 便笺 CRUD ==")
        page.locator(".note-add").click()
        card = page.locator(".note-card")
        check("08 新增便笺出现", card.count() == 1)
        check("09 新建后自动聚焦标题", page.evaluate("document.activeElement?.classList.contains('note-title-input')"))
        page.locator(".note-title-input").fill("买咖啡")
        page.locator(".note-content").fill("燕麦拿铁 ×2\n记得带杯套")
        page.locator(".note-title-input").press("Enter")  # 收起（标题 Enter → toggle）
        page.wait_for_timeout(150)
        check("10 收起态显示标题", "买咖啡" in page.locator(".note-card").inner_text())
        check("11 收起态显示首行摘要", "燕麦拿铁 ×2" in page.locator(".note-card").inner_text())
        cs = page.evaluate("() => { const c = getComputedStyle(document.querySelector('.note-card')); return c.cursor + '/' + c.userSelect; }")
        check("12 收起态 grab 光标 + 防选中", cs == "grab/none", cs)
        page.locator(".note-card").click()  # 展开（收起态点卡片空白处）
        page.wait_for_timeout(100)
        check("13 展开态出现输入框", page.locator(".note-title-input").count() == 1)
        page.locator(".note-del").click()
        page.wait_for_timeout(150)
        check("14 删除便笺", page.locator(".note-card").count() == 0)

        print("== 课表 ==")
        slots = page.locator(".tt-slot")
        check("15 48 个课格（8 节 × 6 天）", slots.count() == 48, str(slots.count()))
        check("16 占位 — 弱化 opacity 0.35",
              page.evaluate("parseFloat(getComputedStyle(document.querySelector('.tt-slot.empty')).opacity)") == 0.35)
        n_today = 0 if today.weekday() == 6 else 8  # 周日 today=null → 0 格
        check("17 今日列高亮" + ("（周日无高亮）" if n_today == 0 else " 8 格"),
              page.locator(".tt-slot.today").count() == n_today,
              str(page.locator(".tt-slot.today").count()))
        slots.nth(0).click()
        edit = page.locator(".tt-edit")
        check("18 点格进入编辑", edit.count() == 1)
        edit.fill("高数")
        edit.press("Enter")
        page.wait_for_timeout(100)
        check("19 Enter 提交课程", slots.nth(0).inner_text() == "高数")
        slots.nth(0).click()
        edit = page.locator(".tt-edit")
        edit.fill("临时代入")
        edit.press("Escape")
        page.wait_for_timeout(100)
        check("20 Esc 取消编辑", slots.nth(0).inner_text() == "高数")

        print("== 持久化 / 折叠 / 快捷键 / 迁移 ==")
        page.keyboard.press("Control+s")
        page.wait_for_timeout(300)
        saved = page.evaluate("localStorage.getItem('piwidget-data')")
        check("21 数据落 localStorage(piwidget-data)", saved and "高数" in saved)
        left_w = page.evaluate("document.querySelector('.left').getBoundingClientRect().width")
        header_y = page.evaluate("document.querySelector('.tt-header').getBoundingClientRect().y")
        page.locator(".tt-toggle").click()
        page.wait_for_timeout(300)  # 等 0fr 过渡
        left_w2 = page.evaluate("document.querySelector('.left').getBoundingClientRect().width")
        header_y2 = page.evaluate("document.querySelector('.tt-header').getBoundingClientRect().y")
        wrap_h2 = page.evaluate("document.querySelector('.tt-grid-wrap').getBoundingClientRect().height")
        check("22 课表折叠：跳到底部同时收缩（左栏撑满、课格隐藏）",
              page.evaluate("document.querySelector('.sheet').classList.contains('collapsed')") and
              left_w2 > left_w + 300 and header_y2 > header_y + 100 and wrap_h2 < 5,
              f"left {left_w:.0f}→{left_w2:.0f}, header y {header_y:.0f}→{header_y2:.0f}, wrap {wrap_h2:.0f}px")
        page.locator(".tt-toggle").click()
        page.wait_for_timeout(300)
        page.keyboard.press("Control+s")
        page.wait_for_timeout(300)
        check("23 Ctrl+S toast「已保存」", "已保存" in page.locator(".toast").inner_text())
        # v1 迁移：单条 note 字符串 → 单条便笺
        page.evaluate("""() => {
            localStorage.setItem('piwidget-data', JSON.stringify(
                { version: 1, note: '旧版便笺内容', timetable: { '1': { '2': '旧课' } } }));
        }""")
        page.reload(wait_until="networkidle")
        page.wait_for_selector(".sheet")
        check("24 v1 数据自动迁移为便笺", "旧版便笺内容" in page.locator(".note-card").inner_text())
        check("25 主题 token --highlight 为 #FFD84D",
              page.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--highlight').trim()")
              .lower() == "#ffd84d")

        print("== 标题栏拖拽 ==")
        drag_regions = page.locator("[data-tauri-drag-region]")
        check("26 拖拽区覆盖品牌字 + 填充区", drag_regions.count() == 2 and
              page.locator(".drag-fill[data-tauri-drag-region]").count() == 1)
        fill_w = page.evaluate("document.querySelector('.drag-fill').getBoundingClientRect().width")
        check("27 填充区占据标题栏空白（> 300px）", fill_w > 300, str(fill_w))
        pin = page.locator(".icon-btn[title='置顶']")
        check("28 初始不置顶（默认 pinned=false）", pin.get_attribute("aria-pressed") == "false")
        pin.click()
        page.wait_for_timeout(150)
        check("29 拖拽区外按钮仍可点击（置顶切换）", pin.get_attribute("aria-pressed") != "false")

        print("== 透明化 ==")
        ghost_btn = page.locator(".icon-btn[title='透明化']")
        check("30 透明化按钮存在且初始未激活", ghost_btn.count() == 1 and
              ghost_btn.get_attribute("aria-pressed") == "false")
        ghost_btn.click()
        page.wait_for_timeout(300)
        check("31 点击后卡片淡出（.sheet.ghost）", page.evaluate("document.querySelector('.sheet').classList.contains('ghost')"))
        page.keyboard.press("Control+s")
        page.wait_for_timeout(300)
        saved2 = page.evaluate("localStorage.getItem('piwidget-data')")
        check("32 透明化状态已持久化", saved2 and '"ghost": true' in saved2)
        ghost_btn.click()  # 复原，不影响截图
        page.wait_for_timeout(300)

        print("== 开机自启 ==")
        auto_btn = page.locator(".icon-btn[title='开机自启']")
        check("33 自启按钮存在且初始未激活", auto_btn.count() == 1 and
              auto_btn.get_attribute("aria-pressed") == "false")
        auto_btn.click()
        page.wait_for_timeout(200)
        check("34 浏览器下点击提示仅桌面端", "仅桌面端" in page.locator(".toast").inner_text())

        print("== 便笺拖拽排序 ==")
        page.evaluate("(d) => localStorage.setItem('piwidget-data', JSON.stringify(d))", {
            "version": 3, "pinned": False, "ghost": False, "timetableCollapsed": False,
            "notes": [
                {"id": "n1", "title": "便笺 A", "content": "a"},
                {"id": "n2", "title": "便笺 B", "content": "b"},
                {"id": "n3", "title": "便笺 C", "content": "c"},
            ],
            "timetables": [{"id": "default", "name": "默认", "cells": {"1": {"1": "高数"}}}],
            "activeTimetableId": "default",
        })
        page.reload(wait_until="networkidle")
        page.wait_for_selector(".sheet")
        page.wait_for_timeout(200)
        page.evaluate("""() => {
            const cards = document.querySelectorAll('.note-card');
            const dt = new DataTransfer();
            cards[0].dispatchEvent(new DragEvent('dragstart', { dataTransfer: dt, bubbles: true, cancelable: true }));
            cards[2].dispatchEvent(new DragEvent('dragover', { dataTransfer: dt, bubbles: true, cancelable: true }));
            cards[2].dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
            cards[0].dispatchEvent(new DragEvent('dragend', { dataTransfer: dt, bubbles: true }));
        }""")
        page.wait_for_timeout(200)
        order = [page.locator(".note-card").nth(i).inner_text().split("\n")[0] for i in range(3)]
        check("35 拖拽 0→2 后顺序为 B/C/A", order == ["便笺 B", "便笺 C", "便笺 A"], str(order))

        print("== 多课表 ==")
        sel = page.locator(".tt-select")

        def opt_texts():
            return [sel.locator("option").nth(i).inner_text() for i in range(sel.locator("option").count())]

        check("36 初始 1 条课表「默认」", opt_texts() == ["默认"], str(opt_texts()))
        page.locator(".tt-mini[title='新建课表']").click()
        page.wait_for_timeout(150)
        check("37 ＋ 新建「课表 2」并自动切换",
              opt_texts() == ["默认", "课表 2"] and
              sel.input_value() == sel.locator("option").nth(1).get_attribute("value"),
              str(opt_texts()))
        slots.nth(0).click()
        page.locator(".tt-edit").fill("化学")
        page.locator(".tt-edit").press("Enter")
        page.wait_for_timeout(100)
        check("38 新课表可填课", slots.nth(0).inner_text() == "化学")
        sel.select_option(index=0)
        page.wait_for_timeout(100)
        check("39 切回「默认」旧课仍在", slots.nth(0).inner_text() == "高数")
        sel.select_option(index=1)
        page.wait_for_timeout(100)
        page.locator(".tt-mini[title='重命名当前课表']").click()
        page.wait_for_selector(".tt-rename")
        page.locator(".tt-rename").fill("这学期")
        page.locator(".tt-rename").press("Enter")
        page.wait_for_timeout(150)
        check("40 ✎ 重命名为「这学期」", opt_texts() == ["默认", "这学期"], str(opt_texts()))
        page.locator(".tt-mini[title='重命名当前课表']").click()
        page.wait_for_selector(".tt-rename")
        page.locator(".tt-rename").fill("   ")
        page.locator(".tt-rename").press("Enter")
        page.wait_for_timeout(150)
        check("41 空名被拒（保持「这学期」）", opt_texts() == ["默认", "这学期"] and
              "名称不能为空" in page.locator(".toast").inner_text(), str(opt_texts()))
        page.locator(".tt-mini[title='删除当前课表']").click()
        page.wait_for_timeout(150)
        check("42 × 删除当前课表，回落「默认」", opt_texts() == ["默认"], str(opt_texts()))
        page.locator(".tt-mini[title='新建课表']").click()
        page.wait_for_timeout(100)
        del_btn = page.locator(".tt-mini[title='删除当前课表']")
        del_btn.click()
        page.wait_for_timeout(100)
        check("43 仅剩 1 条时删除按钮禁用", del_btn.is_disabled())
        page.keyboard.press("Control+s")
        page.wait_for_timeout(300)
        saved3 = page.evaluate("localStorage.getItem('piwidget-data')")
        check("44 多课表结构已持久化", saved3 and '"timetables"' in saved3 and '"activeTimetableId"' in saved3)
        # v2 → v3 迁移：单一 timetable → 「默认」
        page.evaluate("""() => localStorage.setItem('piwidget-data', JSON.stringify(
            { version: 2, notes: [], timetable: { '2': { '3': '旧课' } } }))""")
        page.reload(wait_until="networkidle")
        page.wait_for_selector(".sheet")
        check("45 v2 数据迁移为「默认」课表且旧课保留",
              opt_texts() == ["默认"] and slots.nth(13).inner_text() == "旧课")

        print("== 截图 ==")
        # 演示数据（v3，含自定义名课表）+ 中性桌面底
        demo = {
            "version": 3, "pinned": False, "ghost": False, "timetableCollapsed": False,
            "notes": [
                {"id": "a1", "title": "本周待办", "content": "交周报\n整理实验数据\n预约牙医"},
                {"id": "a2", "title": "购物清单", "content": "燕麦拿铁 x2\n咖啡豆 1kg"},
            ],
            "timetables": [
                {"id": "default", "name": "默认", "cells": {
                    "1": {"1": "高数", "2": "高数", "4": "英语"},
                    "2": {"1": "物理", "3": "物理"},
                    "3": {"2": "化学", "5": "化学"},
                    "4": {"1": "高数", "6": "体育"},
                    "5": {"3": "编程"},
                }},
                {"id": "t2", "name": "这学期", "cells": {
                    "1": {"2": "线性代数"},
                    "3": {"4": "概率论"},
                }},
            ],
            "activeTimetableId": "default",
        }
        page.evaluate("(d) => localStorage.setItem('piwidget-data', JSON.stringify(d))", demo)
        page.reload(wait_until="networkidle")
        page.wait_for_selector(".sheet")
        page.add_style_tag(content=f"html {{ background: {DEMO_BG} !important; }}")
        page.wait_for_timeout(900)  # 等 sweep 动画结束
        shot = ROOT / "docs" / "screenshot.png"
        shot.parent.mkdir(parents=True, exist_ok=True)
        page.screenshot(path=str(shot))
        print(f"  📸 {shot}")

        browser.close()

    print(f"\n{len(passed)} 通过 / {len(failed)} 失败")
    if failed:
        print("失败项:", *failed, sep="\n  - ")
        sys.exit(1)


if __name__ == "__main__":
    main()
