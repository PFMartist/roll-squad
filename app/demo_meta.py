"""演示数据共用的两样东西：**游戏版本信息** + **从解包主表读数据的小工具**。

`make_demo_box.py`（满练度示例箱，打进便携包）与 `make_demo_roster.py`（网页演示的两份池子）
都从这里取，避免两边各写一份版本号或各写一份星级规则。

出新人之后**只改这里的版本常量**，再各跑一遍两个生成脚本即可。

⚠ 别用「char_ID 最大的干员」判断最新：**联动干员的 ID 是提前分配的**，
  例如女神异闻录联动的结城理是 char_4217，反而比 8 月夏日限定的嘉辛塔（char_4237）小。
  所以这里手写权威值（以 PRTS 各干员页的「上线时间」为准），脚本只做存在性核对。
"""

from __future__ import annotations

# ---------------------------------------------------------------- 版本信息

# 最后一批新干员所属的版本更新日期 / 活动名（官方公告里的叫法）
GAME_VERSION = "2026-09-04"
GAME_VERSION_NAME = "石白深蓝之夜（女神异闻录3 Reload 联动）"

# 解包主表快照的抓取日期
SNAPSHOT = "2026-09-22"

# 该批次新增的干员（联动 S.E.E.S.：结城理 / 岳羽由加莉 / 埃癸斯 / 虎狼丸）
NEWEST_BATCH = ["结城理", "岳羽由加莉", "埃癸斯", "虎狼丸"]

# 给使用者看的一句话（两个产物共用同一种说法）
NOTE = (f"示例干员池，游戏数据到 {GAME_VERSION}「{GAME_VERSION_NAME}」，"
        f"解包主表快照 {SNAPSHOT}。**不是任何人的账号数据**。")

# ---------------------------------------------------------------- 主表小工具

PROFS = ["先锋", "近卫", "重装", "狙击", "术师", "医疗", "辅助", "特种"]
PROF_EN2CN = {
    "PIONEER": "先锋", "WARRIOR": "近卫", "TANK": "重装", "SNIPER": "狙击",
    "CASTER": "术师", "MEDIC": "医疗", "SUPPORT": "辅助", "SPECIAL": "特种",
}

# 潜能：低星容易满潜，六星一般只有一两潜
POTENTIAL = {6: 1, 5: 2, 4: 4, 3: 6, 2: 6, 1: 6}

# 与 Rust roster.rs / 前端 tierPass 一致的两条按星级的线
E1_CAP = {3: 55, 4: 60, 5: 70, 6: 80}
MODULE_GATE = {4: 40, 5: 50, 6: 60}
TIER_DESC = ["全部持有", "精一及以上", "精一满级（按星级）", "精二", "模组线（按星级）", "精二且 Lv≥80"]


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
    """满练度 =（精英化段数 - 1, 末段等级上限），直接从主表 phases 读，不硬编码。
    实测全员一致：★6 E2 90 / ★5 E2 80 / ★4 E2 70 / ★3 E1 55 / ★2 E0 30（不能精一）/ ★1 E0 30。"""
    caps = stage_caps(rec)
    if not caps:
        return (0, 1)
    return (len(caps) - 1, caps[-1])


def stage_caps(rec: dict) -> list[int]:
    """各精英化阶段的等级上限，下标 = 精英化等级（★2/★1 只有一段 → 不能精一）。"""
    return [int(p["maxLevel"]) for p in (rec.get("phases") or [])
            if isinstance(p, dict) and p.get("maxLevel")]


def tier_pass(rarity: int, elite: int, level: int, tier: int) -> bool:
    """六个档位的判定，与 Rust roster.rs 的 tier_pass / 前端 app.js 的 tierPass 同一套规则。"""
    if tier == 2:      # 精一满级（按星级）；E2 一律算过（精英化会重置等级）
        cap = E1_CAP.get(rarity)
        return elite >= 2 or (elite >= 1 and cap is not None and level >= cap)
    if tier == 4:      # 模组线（按星级）
        line = MODULE_GATE.get(rarity)
        return line is not None and elite >= 2 and level >= line
    e, l = {1: (1, 0), 3: (2, 0), 5: (2, 80)}.get(tier, (0, 0))
    return elite >= e and level >= l


def iter_operators(table: dict):
    """主表 → 全部可获取干员（八个职业、非未开放、char_ 开头），产出 (cid, name, rarity, rec)。"""
    for cid, rec in table.items():
        if not isinstance(rec, dict) or not cid.startswith("char_"):
            continue
        if rec.get("profession") not in PROF_EN2CN or rec.get("isNotObtainable"):
            continue
        name = (rec.get("name") or "").strip()
        rar = rarity_of(rec)
        if not name or not rar:
            continue
        yield cid, name, rar, rec
