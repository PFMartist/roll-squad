"""构建守卫：演示池里出现的每个干员，都必须有随包头像。

为什么要有这条 —— 网页版（GitHub Pages）放弃"运行时联网补图"之后，池子和头像是两份
**不同源**的构建快照：`web/assets/demo_roster.js` 是提交进仓库的，`web/assets/avatars/`
是 CI 现从 `app/devdata/avatars/` 转出来的。两者一旦不同步（池里加了人、图没跟上），
演示页上那些干员就永远只剩职业色块，而且**没有任何兜底**。所以在构建阶段直接判死。

用法：
    python app/check_demo_avatars.py        # 缺图 → 退出码 1
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# 输出被管道捕获时 Windows Python 会用 GBK，中文/符号直接崩在 print 上（与其它脚本同款处理）
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

ROOT = Path(__file__).resolve().parent          # app/
WS = ROOT.parent                                # 仓库根
ROSTER = WS / "web" / "assets" / "demo_roster.js"
MANIFEST = WS / "web" / "assets" / "avatars" / "manifest.json"
SRC = ROOT / "devdata" / "avatars"              # 头像源（PNG）

# 池子明显缩水 = 解析规则跟生成脚本对不上了，这时也要报错，别让守卫静默失效
MIN_POOL = 400

# demo_roster.js 里的干员是紧凑数组 [名字, 星级, 精英化, 等级, 职业序号, 潜能]，
# 按 `["名字", <数字>` 这个形状认人 —— profs / newest 那些纯字符串数组不会被误认。
OP = re.compile(r'\[\s*"((?:[^"\\]|\\.)*)"\s*,\s*\d')


def sanitize(name: str) -> str:
    """与 Rust 的 paths.rs::avatar_file、前端的 avatarKey 同一条规则（三处必须一致）。"""
    return "".join("_" if c in '\\/:*?"<>|' else c for c in name)


def main() -> int:
    if not MANIFEST.is_file():
        print(f"✗ 没有 {MANIFEST} —— 先跑一遍 python app/make_avatar_assets.py")
        return 1

    # utf-8-sig：记事本 / PowerShell 5.1 存过的文件会带 BOM，别为此整份读不出来
    try:
        names = json.loads(MANIFEST.read_text(encoding="utf-8-sig")).get("names") or []
    except Exception as e:
        print(f"✗ 读不了 {MANIFEST}：{e}")
        return 1
    have = {sanitize(n) for n in names}

    if not ROSTER.is_file():
        print(f"✗ 没有 {ROSTER}")
        return 1
    text = ROSTER.read_text(encoding="utf-8-sig")
    # 名字在文件里是 JSON 字符串，转义要还原（json.loads 比手写 unescape 稳）
    pool = {sanitize(json.loads(f'"{m}"')) for m in OP.findall(text)}

    if len(pool) < MIN_POOL:
        print(f"✗ 只从 {ROSTER.name} 里认出 {len(pool)} 名干员（应 ≥ {MIN_POOL}）")
        print("  解析规则和生成脚本（app/make_demo_roster.py）对不上了，先修守卫本身")
        return 1

    missing = sorted(n for n in pool if n not in have)
    if missing:
        head = "、".join(missing[:8]) + ("…" if len(missing) > 8 else "")
        print(f"✗ 演示池 {len(pool)} 人里有 {len(missing)} 人没有随包头像：{head}")
        print(f"  头像源目录：{SRC}（把缺的 PNG 放进去，再跑 make_avatar_assets.py）")
        return 1

    print(f"✓ 演示池 {len(pool)} 人全部有随包头像（清单 {len(names)} 张）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
