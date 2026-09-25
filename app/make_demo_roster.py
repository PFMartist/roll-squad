"""生成 `web/assets/demo_roster.js` —— 在线演示（GitHub Pages）用的干员池，**两份**：

  · `spread`（默认）= **练度洒开**：按名字 crc32 取一档练度，刻意让人分布在各阶段
    （有人没精一、有人精一没满、有人精二但没到模组线），六个档位的人数才互不相同，
    专门用来演示「筛选档位」这个功能：429 / 357 / 273 / 227 / 137 / 71。
  · `max` = **全干员满练度**：各星级封顶（★6 E2 90 / ★5 E2 80 / ★4 E2 70 / ★3 E1 55 / ★2·★1 E0 30），
    人人都在上限，所以「精一+」与「精一满级」、「精二」与「模组线」人数必然相同 —— 这是它的性质，不是 bug。

两份都是**合成数据**（不是任何人的账号数据），也都不含立绘/头像：卡片走职业色块 + 名字首字。
版本信息与星级规则在 `app/demo_meta.py`，与 `make_demo_box.py` 共用。

用法：
    python app/make_demo_roster.py <character_table.json> [--out web/assets/demo_roster.js]
"""

from __future__ import annotations

import argparse
import json
import sys
import zlib
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

ROOT = Path(__file__).resolve().parent            # app/
WS = ROOT.parent                                  # 仓库根
DEFAULT_OUT = WS / "web" / "assets" / "demo_roster.js"

sys.path.insert(0, str(ROOT))
import demo_meta as M       # noqa: E402

# 洒开用的练度候选（精英化, 等级），重复项 = 权重。按名字哈希取一条
PROFILES: dict[int, list[tuple[int, int]]] = {
    6: [(2, 90), (2, 90), (2, 80), (2, 60), (2, 45), (2, 30), (2, 1), (1, 80), (1, 60), (1, 1), (0, 1)],
    5: [(2, 80), (2, 80), (2, 70), (2, 50), (2, 30), (2, 1), (1, 70), (1, 55), (1, 1), (0, 1)],
    4: [(2, 70), (2, 60), (2, 45), (2, 40), (2, 30), (2, 1), (1, 60), (1, 40), (1, 1), (0, 1)],
    3: [(1, 55), (1, 55), (1, 45), (1, 30), (1, 15), (1, 1), (0, 1)],
    2: [(0, 30), (0, 30), (0, 20), (0, 10), (0, 1)],
    1: [(0, 30), (0, 30), (0, 20), (0, 10), (0, 1)],
}

POOLS = [
    ("spread", "演示池 · 随机练度（看档位差别）"),
    ("max", "演示池 · 全干员满练度"),
]


def build(table: dict, mode: str) -> list[list]:
    """→ [[名字, 星级, 精英化, 等级, 职业序号, 潜能], ...]"""
    ops = []
    for _cid, name, rar, rec in M.iter_operators(table):
        if mode == "max":
            elite, level = M.max_train(rec)
        else:
            cand = PROFILES.get(rar) or [(0, 1)]
            elite, level = cand[zlib.crc32(name.encode("utf-8")) % len(cand)]
        ops.append([name, rar, elite, level,
                    M.PROFS.index(M.PROF_EN2CN[rec["profession"]]), M.POTENTIAL.get(rar, 1)])
    ops.sort(key=lambda o: (-o[1], o[0]))
    return ops


def js_array(ops: list[list]) -> str:
    return ",\n".join(
        "      [" + ",".join([json.dumps(o[0], ensure_ascii=False)] + [str(v) for v in o[1:]]) + "]"
        for o in ops
    )


def counts_of(ops: list[list]) -> list[int]:
    return [sum(1 for o in ops if M.tier_pass(o[1], o[2], o[3], t)) for t in range(6)]


def main() -> int:
    ap = argparse.ArgumentParser(description="生成在线演示的两份干员池")
    ap.add_argument("table", help="character_table.json 路径（解包主表）")
    ap.add_argument("--out", default=str(DEFAULT_OUT), help=f"输出路径（默认 {DEFAULT_OUT}）")
    args = ap.parse_args()

    table_path = Path(args.table)
    if not table_path.is_file():
        sys.exit(f"没有这份主表：{table_path}\n  —— 先用 tools/cultivation_plan.py --refresh 拉一份")
    out_path = Path(args.out)
    table = json.loads(table_path.read_text(encoding="utf-8"))

    built = {key: build(table, key) for key, _label in POOLS}

    out_path.parent.mkdir(parents=True, exist_ok=True)
    pools_js = ",\n".join(
        f'    {key}: {{\n      label: {json.dumps(label, ensure_ascii=False)},\n'
        f"      ops: [\n{js_array(built[key])},\n      ],\n    }}"
        for key, label in POOLS
    )
    out_path.write_text(
        "/* 演示干员池：全游戏可获取干员，由 app/make_demo_roster.py 生成，别手改。\n"
        f"   对应游戏版本：{M.GAME_VERSION}「{M.GAME_VERSION_NAME}」· 主表快照抓取于 {M.SNAPSHOT}\n"
        f"   该批次新增：{'、'.join(M.NEWEST_BATCH)} —— 游戏出新人后重新跑一遍生成脚本即可。\n"
        "   两份池子：spread = 练度洒开（演示六个档位的差别）；max = 全干员满练度。\n"
        "   格式：[名字, 星级, 精英化, 等级, 职业序号, 潜能]，职业序号见 profs。\n"
        "   练度都是**合成**数据（不是任何人的账号数据），也不含任何立绘/头像，\n"
        "   卡片走职业色块 + 名字首字。 */\n"
        "window.DEMO_ROSTER = {\n"
        f"  version: {json.dumps(M.GAME_VERSION)},\n"
        f"  versionName: {json.dumps(M.GAME_VERSION_NAME, ensure_ascii=False)},\n"
        f"  snapshot: {json.dumps(M.SNAPSHOT)},\n"
        f"  newest: {json.dumps(M.NEWEST_BATCH, ensure_ascii=False)},\n"
        f"  profs: {json.dumps(M.PROFS, ensure_ascii=False)},\n"
        "  pools: {\n" + pools_js + ",\n  },\n"
        "};\n",
        encoding="utf-8", newline="\n",
    )

    print(f"写出 {out_path}  {out_path.stat().st_size/1024:.0f} KB")
    print(f"版本 {M.GAME_VERSION}「{M.GAME_VERSION_NAME}」· 快照 {M.SNAPSHOT}")
    for key, label in POOLS:
        ops = built[key]
        cs = counts_of(ops)
        print(f"  [{key}] {label}：{len(ops)} 名")
        print("       档位人数：" + "  ".join(f"t{t}={n}" for t, n in enumerate(cs))
              + ("（t2=t1、t4=t3 是满练度的必然结果）" if len(set(cs)) < len(cs) else "（六档互不相同）"))
        print("       星级分布：" + "  ".join(f"★{r}={sum(1 for o in ops if o[1] == r)}" for r in range(1, 7)))

    # 自检：练度不能超过该干员自己星级的阶段上限
    recs = {n: r for _c, n, _ra, r in M.iter_operators(table)}
    bad = []
    for key, _label in POOLS:
        for o in built[key]:
            caps = M.stage_caps(recs[o[0]])
            if o[2] >= len(caps) or o[3] > caps[o[2]]:
                bad.append(f"{key}:{o[0]}(E{o[2]} Lv{o[3]})")
    print("  练度合法性：" + ("全部在阶段上限内 ✓" if not bad else f"越界 ✗ {bad[:5]}"))

    names = {o[0] for o in built["spread"]}
    missing = [n for n in M.NEWEST_BATCH if n not in names]
    print("  最新批次核对：" + ("、".join(M.NEWEST_BATCH) + " 都在池子里 ✓" if not missing
                                else f"⚠ 不在池子里：{'、'.join(missing)}"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
