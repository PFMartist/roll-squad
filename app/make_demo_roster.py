"""生成 `web/assets/demo_roster.js` —— 在线演示（GitHub Pages）用的**全游戏干员池**。

网页演示没有后端、没有 box，桌面版那套「box × 职业索引」在这里没有输入，
所以从解包主表把**全部可获取干员**（八个职业、非 TOKEN/TRAP/未开放）连星级、职业烤成静态数据。

练度是**合成**的，规则两条：
  · 星级决定上限（主表 phases 实测：1★/2★ 只有一段、封顶 E0 Lv30，不能精英化；
    3★ 到 E1 55；4★/5★/6★ 到 E2 60/80/90）
  · 名字的 crc32 决定这个人练到哪一档 —— 确定性哈希，同一个人每次生成都一样，
    而且刻意洒开（有人没精一、有人精一没满、有人精二了但没到模组线），
    这样 6 个档位的人数才互不相同，演示才有意义。
**不是任何人的账号数据**，也不含任何立绘/头像（卡片走职业色块 + 名字首字）。

用法：
    python app/make_demo_roster.py <character_table.json> [--out web/assets/demo_roster.js]

主表从哪来：`tools/cultivation_plan.py --refresh` 会从社区解包仓
Kengxxiao/ArknightsGameData 拉一份到 refs/cultivation/character_table.json。
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

# ---------------------------------------------------------------- 游戏版本
# 版本信息集中在 app/demo_meta.py（跟 make_demo_box.py 共用一份），出新人只改那儿。
sys.path.insert(0, str(ROOT))
import demo_meta as META       # noqa: E402

GAME_VERSION = META.GAME_VERSION
GAME_VERSION_NAME = META.GAME_VERSION_NAME
SNAPSHOT = META.SNAPSHOT
NEWEST_BATCH = META.NEWEST_BATCH

PROFS = ["先锋", "近卫", "重装", "狙击", "术师", "医疗", "辅助", "特种"]
PROF_EN2CN = {
    "PIONEER": "先锋", "WARRIOR": "近卫", "TANK": "重装", "SNIPER": "狙击",
    "CASTER": "术师", "MEDIC": "医疗", "SUPPORT": "辅助", "SPECIAL": "特种",
}

# 各星级的练度候选（精英化, 等级），重复项 = 权重。按名字哈希取一条
PROFILES: dict[int, list[tuple[int, int]]] = {
    6: [(2, 90), (2, 90), (2, 80), (2, 60), (2, 45), (2, 30), (2, 1), (1, 80), (1, 60), (1, 1), (0, 1)],
    5: [(2, 80), (2, 80), (2, 70), (2, 50), (2, 30), (2, 1), (1, 70), (1, 55), (1, 1), (0, 1)],
    4: [(2, 60), (2, 60), (2, 45), (2, 40), (2, 30), (2, 1), (1, 60), (1, 40), (1, 1), (0, 1)],
    3: [(1, 55), (1, 55), (1, 45), (1, 30), (1, 15), (1, 1), (0, 1)],
    2: [(0, 30), (0, 30), (0, 20), (0, 10), (0, 1)],
    1: [(0, 30), (0, 30), (0, 20), (0, 10), (0, 1)],
}
POTENTIAL = {6: 1, 5: 2, 4: 4, 3: 6, 2: 6, 1: 6}     # 低星容易满潜，六星一般一两潜

# 与 Rust roster.rs / 前端 app.js 的 tierPass 一致的两条按星级的线
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


def id_num(cid: str) -> int:
    try:
        return int(cid.split("_")[1])
    except (IndexError, ValueError):
        return -1


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
    ap = argparse.ArgumentParser(description="生成在线演示用的全游戏干员池")
    ap.add_argument("table", help="character_table.json 路径（解包主表）")
    ap.add_argument("--out", default=str(DEFAULT_OUT), help=f"输出路径（默认 {DEFAULT_OUT}）")
    args = ap.parse_args()

    table_path = Path(args.table)
    if not table_path.is_file():
        sys.exit(f"没有这份主表：{table_path}\n  —— 先用 tools/cultivation_plan.py --refresh 拉一份")
    out_path = Path(args.out)

    table = json.loads(table_path.read_text(encoding="utf-8"))
    ops = []
    newest = ("", -1)
    for cid, rec in table.items():
        if not isinstance(rec, dict):
            continue
        prof_cn = PROF_EN2CN.get(rec.get("profession") or "")
        if not prof_cn or rec.get("isNotObtainable"):
            continue
        name = (rec.get("name") or "").strip()
        rar = rarity_of(rec)
        if not name or not rar:
            continue
        if cid.startswith("char_") and id_num(cid) > newest[1]:
            newest = (name, id_num(cid))
        cand = PROFILES.get(rar) or [(0, 1)]
        elite, level = cand[zlib.crc32(name.encode("utf-8")) % len(cand)]
        ops.append((name, rar, elite, level, PROFS.index(prof_cn), POTENTIAL.get(rar, 1)))

    ops.sort(key=lambda o: (-o[1], o[0]))

    body = ",\n".join(
        "  [" + ",".join([json.dumps(o[0], ensure_ascii=False)] + [str(v) for v in o[1:]]) + "]"
        for o in ops
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        "/* 演示干员池：全游戏可获取干员，由 app/make_demo_roster.py 生成，别手改。\n"
        f"   对应游戏版本：{GAME_VERSION}「{GAME_VERSION_NAME}」· 主表快照抓取于 {SNAPSHOT}\n"
        f"   该批次新增：{'、'.join(NEWEST_BATCH)} —— 游戏出新人后重新跑一遍生成脚本即可。\n"
        "   格式：[名字, 星级, 精英化, 等级, 职业序号, 潜能]，职业序号见 profs。\n"
        "   练度是按名字哈希洒开的**合成**数据（不是任何人的账号数据），只为让 6 个档位人数互不相同；\n"
        "   这里也不含任何立绘/头像，卡片走职业色块 + 名字首字。 */\n"
        "window.DEMO_ROSTER = {\n"
        f"  version: {json.dumps(GAME_VERSION)},\n"
        f"  versionName: {json.dumps(GAME_VERSION_NAME, ensure_ascii=False)},\n"
        f"  snapshot: {json.dumps(SNAPSHOT)},\n"
        f"  newest: {json.dumps(NEWEST_BATCH, ensure_ascii=False)},\n"
        f"  profs: {json.dumps(PROFS, ensure_ascii=False)},\n"
        "  ops: [\n" + body + ",\n  ],\n"
        "};\n",
        encoding="utf-8", newline="\n",
    )

    counts = [sum(1 for o in ops if tier_pass(o[1], o[2], o[3], t)) for t in range(6)]
    names = {o[0] for o in ops}
    print(f"写出 {out_path}  {out_path.stat().st_size/1024:.0f} KB")
    print(f"干员 {len(ops)} 名  ·  版本 {GAME_VERSION}「{GAME_VERSION_NAME}」·  快照 {SNAPSHOT}")
    print("  档位人数：" + "  ".join(f"t{t}={n}" for t, n in enumerate(counts)))
    print("  （0 全部 / 1 精一+ / 2 精一满级 / 3 精二 / 4 模组线 / 5 精二 Lv≥80）")
    print("  星级分布：" + "  ".join(f"★{r}={sum(1 for o in ops if o[1] == r)}" for r in range(1, 7)))
    print("  职业分布：" + "  ".join(f"{p}={sum(1 for o in ops if PROFS[o[4]] == p)}" for p in PROFS))

    bad = [t for t in range(1, 6) if counts[t] > counts[t - 1]] + [t for t in range(6) if counts[t] == 0]
    print("  包含关系与空档位：" + ("全部正常 ✓" if not bad else f"有问题 ✗ {bad}"))
    if len(set(counts)) != len(counts):
        print("  ⚠ 有档位人数相同 —— 演示时不易看出区别，可调整 PROFILES 的权重")

    missing = [n for n in NEWEST_BATCH if n not in names]
    if missing:
        print(f"  ⚠ 声明的「{GAME_VERSION}」批次里，这些干员不在池子里：{'、'.join(missing)}")
        print("     —— 主表可能没刷到那一版，或者名字写错了")
    else:
        print(f"  最新批次核对：{'、'.join(NEWEST_BATCH)} 都在池子里 ✓")
    print(f"  （参考）快照里 char_ID 最大的是 {newest[0]}（char_{newest[1]}）——"
          " 联动干员 ID 提前分配，别拿它当\"最新\"")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
