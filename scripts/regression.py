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
        page.locator(".tt-toggle").click()
        check("22 课表折叠", page.evaluate("document.querySelector('.sheet').classList.contains('collapsed')"))
        page.locator(".tt-toggle").click()
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

        print("== 截图 ==")
        # 演示数据 + 中性桌面底
        demo = {
            "version": 2, "pinned": True, "timetableCollapsed": False,
            "notes": [
                {"id": "a1", "title": "本周待办", "content": "交周报\n整理实验数据\n预约牙医"},
                {"id": "a2", "title": "购物清单", "content": "燕麦拿铁 x2\n咖啡豆 1kg"},
            ],
            "timetable": {
                "1": {"1": "高数", "2": "高数", "4": "英语"},
                "2": {"1": "物理", "3": "物理"},
                "3": {"2": "化学", "5": "化学"},
                "4": {"1": "高数", "6": "体育"},
                "5": {"3": "编程"},
            },
        }
        page.evaluate("(d) => localStorage.setItem('piwidget-data', JSON.stringify(d))", demo)
        page.reload(wait_until="networkidle")
        page.wait_for_selector(".sheet")
        page.add_style_tag(content=f"html {{ background: {DEMO_BG} !important; }}")
        page.wait_for_timeout(900)  # 等 sweep 动画结束
        shot = ROOT / "docs" / "screenshot.png"
        page.screenshot(path=str(shot))
        print(f"  📸 {shot}")

        browser.close()

    print(f"\n{len(passed)} 通过 / {len(failed)} 失败")
    if failed:
        print("失败项:", *failed, sep="\n  - ")
        sys.exit(1)


if __name__ == "__main__":
    main()
