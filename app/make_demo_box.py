"""生成**全干员满练度**的示例干员箱 `box_demo.json`（打进 Release 的便携包）。

跟 `make_demo_roster.py` 的分工：
  · 这一份 = **满练度**，给便携包当"打开就能看见东西"的底子，练度就是各星级的封顶值；
    代价是「精一及以上」与「精一满级」、「精二」与「模组线」人数必然两两相同（人人都在上限）。
  · 网页演示那份（`web/assets/demo_roster.js`）= 两份池子：练度洒开的 + 满练度的，
    前者专门用来看出六个档位的筛选差别。
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

ROOT = Path(__file__).resolve().parent
DEFAULT_OUT = ROOT / "devdata" / "box_demo.json"

sys.path.insert(0, str(ROOT))
import demo_meta as M       # noqa: E402


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
    for cid, name, rar, rec in M.iter_operators(table):
        elite, level = M.max_train(rec)
        caps_seen.setdefault(rar, set()).add((elite, level))
        ops.append({
            "id": cid, "name": name, "rarity": rar,
            "elite": elite, "level": level,
            "potential": M.POTENTIAL.get(rar, 1), "own": True,
        })
    ops.sort(key=lambda o: (-o["rarity"], o["name"]))

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        json.dumps({
            "_note": M.NOTE + " 满练度只是让它开箱有东西可抽；"
                     "换成自己的：MAA 里跑一次干员识别导出 OperBoxData.json，改名成 box_日期.json 放进同目录，"
                     "再在界面顶部的干员档案下拉里选中它。",
            "done": True,
            "syncTime": None,
            "own_opers": ops,
        }, ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8", newline="\n",
    )

    counts = [sum(1 for o in ops if M.tier_pass(o["rarity"], o["elite"], o["level"], t)) for t in range(6)]
    print(f"写出 {out_path}  {out_path.stat().st_size/1024:.0f} KB")
    print(f"干员 {len(ops)} 名  ·  版本 {M.GAME_VERSION}「{M.GAME_VERSION_NAME}」·  快照 {M.SNAPSHOT}")
    print("  满练度（各星级封顶，主表实测全员一致）：" +
          "  ".join(f"★{r}={sorted(caps_seen[r])[0]}" for r in sorted(caps_seen, reverse=True)))
    print("  档位人数：" + "  ".join(f"t{t}={n}" for t, n in enumerate(counts)))
    print("  （0 全部 / 1 精一+ / 2 精一满级 / 3 精二 / 4 模组线 / 5 精二 Lv≥80）")
    print("  星级分布：" + "  ".join(f"★{r}={sum(1 for o in ops if o['rarity'] == r)}" for r in range(1, 7)))

    bad = [o["name"] for o in ops if (o["elite"], o["level"]) not in caps_seen[o["rarity"]]]
    print("  练度合法性：" + ("全部在封顶值内 ✓" if not bad else f"越界 ✗ {bad[:5]}"))

    missing = [n for n in M.NEWEST_BATCH if n not in {o["name"] for o in ops}]
    print("  最新批次核对：" + ("、".join(M.NEWEST_BATCH) + " 都在池子里 ✓" if not missing
                                else f"⚠ 不在池子里：{'、'.join(missing)}"))

    pairs = [f"t{t}=t{t - 1}" for t in range(1, 6) if counts[t] == counts[t - 1]]
    if pairs:
        print(f"  提示：满练度池里 {'、'.join(pairs)} 必然相同（人人都在上限）——"
              " 要演示六个档位的差别，用网页那份的 spread 池")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
