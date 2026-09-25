"""生成**全干员满练度**的示例干员箱 `box_demo.json`（打进 Release 的便携包）。

跟 `make_demo_roster.py` 的分工：
  · 这一份 = **满练度**，给便携包当"打开就能看见东西"的底子，练度就是各星级的封顶值；
    代价是「精一及以上」与「精一满级」、「精二」与「模组线」人数必然两两相同（人人都在上限）。
  · 网页演示那份（`web/assets/demo_roster.js`）= **练度洒开**，专门用来看出六个档位的筛选差别。
两份都是合成数据，不含任何人的账号信息，也不含立绘。

满练度不是拍脑袋，直接读主表 `phases`：星级 →（精英化段数-1, 末段 maxLevel），实测全员一致：
    ★6 = E2 90 / ★5 = E2 80 / ★4 = E2 70 / ★3 = E1 55 / ★2 = E0 30（不能精一）/ ★1 = E0 30

用法：
    python app/make_demo_box.py <character_table.json> [--out app/devdata/box_demo.json]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

sys.path.insert(0, str(Path(__file__).resolve().parent))
import demo_meta as META       # noqa: E402

ROOT = Path(__file__).resolve().parent
WS = ROOT.parent
DEFAULT_OUT = ROOT / "devdata" / "box_demo.json"

PROF_EN2CN = {
    "PIONEER": "先锋", "WARRIOR": "近卫", "TANK": "重装", "SNIPER": "狙击",
    "CASTER": "术师", "MEDIC": "医疗", "SUPPORT": "辅助", "SPECIAL": "特种",
}
POTENTIAL = {6: 1, 5: 2, 4: 4, 3: 6, 2: 6, 1: 6}     # 低星容易满潜，六星一般一两潜

# 与 Rust roster.rs / 前端 tierPass 一致的两条按星级的线（用来核对档位人数）
E1_CAP = {3: 55, 4: 60, 5: 70, 6: 80}
MODULE_GATE = {4: 40, 5: 50, 6: 60}


def rarity_of(rec: dict) -> int:
    raw = rec.get("rarity")
    if isinstance(raw, int):
        return raw
    if isinstance(raw, str) and raw.startswith("TIER_"):
        try:
            return int(raw.split("_", 1)[1])
        except ValueError:
            return 0
    return 0


def max_train(rec: dict) -> tuple[int, int]:
    """满练度 =（精英化段数 - 1, 末段等级上限），直接从主表 phases 读，不硬编码。"""
    phases = [p for p in (rec.get("phases") or []) if isinstance(p, dict) and p.get("maxLevel")]
    if not phases:
        return (0, 1)
    return (len(phases) - 1, int(phases[-1]["maxLevel"]))


def tier_pass(rar: int, elite: int, level: int, tier: int) -> bool:
    if tier == 2:      # 精一满级（按星级）；E2 一律算过（精英化会重置等级）
        cap = E1_CAP.get(rar)
        return elite >= 2 or (elite >= 1 and cap is not None and level >= cap)
    if tier == 4:      # 模组线（按星级）
        line = MODULE_GATE.get(rar)
        return line is not None and elite >= 2 and level >= line
    e, l = {1: (1, 0), 3: (2, 0), 5: (2, 80)}.get(tier, (0, 0))
    return elite >= e and level >= l


def main() -> int:
    ap = argparse.ArgumentParser(description="生成全干员满练度的示例干员箱")
    ap.add_argument("table", help="character_table.json 路径（解包主表）")
    ap.add_argument("--out", default=str(DEFAULT_OUT), help=f"输出路径（默认 {DEFAULT_OUT}）")
    args = ap.parse_args()

    table_path = Path(args.table)
    if not table_path.is_file():
        sys.exit(f"没有这份主表：{table_path}\n  —— 先用 tools/cultivation_plan.py --refresh 拉一份")
    out_path = Path(args.out)

    table = json.loads(table_path.read_text(encoding="utf-8"))
    ops, caps_seen = [], {}
    for cid, rec in table.items():
        if not isinstance(rec, dict) or not cid.startswith("char_"):
            continue
        if rec.get("profession") not in PROF_EN2CN or rec.get("isNotObtainable"):
            continue
        name = (rec.get("name") or "").strip()
        rar = rarity_of(rec)
        if not name or not rar:
            continue
        elite, level = max_train(rec)
        caps_seen.setdefault(rar, set()).add((elite, level))
        ops.append({
            "id": cid, "name": name, "rarity": rar,
            "elite": elite, "level": level,
            "potential": POTENTIAL.get(rar, 1), "own": True,
        })

    ops.sort(key=lambda o: (-o["rarity"], o["name"]))

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        json.dumps({
            "_note": META.NOTE + " 满练度只是让它开箱有东西可抽；"
                     "换成自己的：MAA 里跑一次干员识别导出 OperBoxData.json，改名成 box_日期.json 放进同目录，"
                     "再在界面顶部的干员档案下拉里选中它。",
            "done": True,
            "syncTime": None,
            "own_opers": ops,
        }, ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8", newline="\n",
    )

    counts = [sum(1 for o in ops if tier_pass(o["rarity"], o["elite"], o["level"], t)) for t in range(6)]
    print(f"写出 {out_path}  {out_path.stat().st_size/1024:.0f} KB")
    print(f"干员 {len(ops)} 名  ·  版本 {META.GAME_VERSION}「{META.GAME_VERSION_NAME}」·  快照 {META.SNAPSHOT}")
    print("  满练度（各星级封顶，主表实测全员一致）：" +
          "  ".join(f"★{r}={sorted(caps_seen[r])[0]}" for r in sorted(caps_seen, reverse=True)))
    print("  档位人数：" + "  ".join(f"t{t}={n}" for t, n in enumerate(counts)))
    print("  （0 全部 / 1 精一+ / 2 精一满级 / 3 精二 / 4 模组线 / 5 精二 Lv≥80）")
    print("  星级分布：" + "  ".join(f"★{r}={sum(1 for o in ops if o['rarity'] == r)}" for r in range(1, 7)))

    # 自检：每个人都不能超过自己星级的封顶
    bad = [o["name"] for o in ops if (o["elite"], o["level"]) not in caps_seen[o["rarity"]]]
    print("  练度合法性：" + ("全部在封顶值内 ✓" if not bad else f"越界 ✗ {bad[:5]}"))

    missing = [n for n in META.NEWEST_BATCH if n not in {o["name"] for o in ops}]
    if missing:
        print(f"  ⚠ 声明的「{META.GAME_VERSION}」批次里，这些干员不在池子里：{'、'.join(missing)}")
    else:
        print(f"  最新批次核对：{'、'.join(META.NEWEST_BATCH)} 都在池子里 ✓")

    pairs = [t for t in range(1, 6) if counts[t] == counts[t - 1]]
    if pairs:
        print(f"  提示：满练度池里 t{pairs} 与上一档人数必然相同（人人都在上限）——"
              " 要演示六个档位的差别，用网页那份 web/assets/demo_roster.js")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
