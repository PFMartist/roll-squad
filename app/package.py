"""打便携包：编译 → 组装「随机编队\\{随机编队.exe, data\\}」→ 离线化（职业索引 + 头像预热）。

打包前会做的事情（都是为了"断网也能用"）：
  · 把 PRTS 职业索引一起放进 data\\（11 KB，覆盖全部干员）
  · 跑一次头像全量预热（约 13 MB，之后抽签零联网）

用法：
    python app/package.py                     # 默认：用 app/devdata 的数据，输出到 dist/
    python app/package.py --box box_20260925.json   # 指定用哪份干员池（默认取最新的一份）
    python app/package.py --no-avatars        # 不预热头像（包小，但首次抽签要联网补图）
    python app/package.py --no-build          # 跳过 cargo build（用现有 release 产物）
    python app/package.py --refresh-index     # 打包前重新抓一次 PRTS 职业索引
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

# 输出被管道/重定向捕获时，Windows Python 会用 GBK 当 stdout 编码，
# 脚本里的 ✓ ⚠ 会直接 UnicodeEncodeError 崩在 print 上（与 tools/maalib.py 同款处理）
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

ROOT = Path(__file__).resolve().parent           # app/
WS = ROOT.parent                                 # 工作区根
EXE_NAME = "随机编队"
SRC_EXE = ROOT / "src-tauri" / "target" / "release" / "roll-squad.exe"


def run(cmd: list[str], **kw) -> int:
    print("  $", " ".join(str(c) for c in cmd))
    return subprocess.run(cmd, **kw).returncode


def build() -> None:
    print("=== 1/5 编译 release ===")
    env = None
    rc = run(["cargo", "build", "--release"], cwd=ROOT / "src-tauri")
    if rc != 0:
        sys.exit("cargo build 失败")
    if not SRC_EXE.is_file():
        sys.exit(f"没找到产物：{SRC_EXE}")
    print(f"  ✓ {SRC_EXE.name}  {SRC_EXE.stat().st_size/1024/1024:.1f} MB")


def pick_source(data: Path, box: str | None) -> Path:
    if box:
        p = data / box
        if not p.is_file():
            sys.exit(f"没有这份 box：{p}")
        return p
    cands = sorted([p for p in data.glob("box*.json")], reverse=True)
    if not cands:
        sys.exit(f"{data} 下没有 box*.json —— 先用 MAA 扫一次干员池（或 --box 指定）")
    return cands[0]


def refresh_index(data: Path) -> None:
    """重新抓一次 PRTS 职业索引。复用了配套 Python 版 tools/roster.py 的解析逻辑
    （它写到 refs/），抓完再拷进数据目录 —— 避免在 Python 与 Rust 两边各维护一份抓取代码。

    本仓只有桌面版，没有 tools\\ —— 那就跳过刷新，沿用 data\\prts_professions.json
    （它是随包预置的全量索引；真缺人时运行时也会自己联网补）。"""
    helper = WS / "tools" / "roster.py"
    if not helper.is_file():
        print("=== 刷新 PRTS 职业索引：跳过 ===")
        print(f"  · 没有 {helper}（本仓不含配套 Python 版），沿用现有索引")
        return
    print("=== 刷新 PRTS 职业索引 ===")
    rc = run([sys.executable, str(helper), "--refresh"], cwd=WS)
    if rc != 0:
        print("  ⚠ 刷新失败，沿用现有索引")
        return
    fresh = WS / "refs" / "prts_professions.json"
    if fresh.is_file():
        shutil.copy2(fresh, data / "prts_professions.json")
        n = len(json.loads(fresh.read_text(encoding="utf-8")))
        print(f"  ✓ 索引已更新：{n} 名干员")


def assemble(out: Path, data: Path, box_path: Path, with_avatars: bool) -> Path:
    print("=== 组装目录 ===")
    app_dir = out / EXE_NAME
    data_dir = app_dir / "data"
    if app_dir.exists():
        try:
            shutil.rmtree(app_dir)
        except PermissionError:
            sys.exit(f"删不掉 {app_dir}\n  —— 大概率是那个 app 还开着（exe 被占用）。关掉再打包。")
    data_dir.mkdir(parents=True)

    shutil.copy2(SRC_EXE, app_dir / f"{EXE_NAME}.exe")
    shutil.copy2(box_path, data_dir / box_path.name)

    # 职业索引永远要带（11 KB，保证离线也有全量职业）
    src = data / "prts_professions.json"
    if not src.is_file():
        src = WS / "refs" / "prts_professions.json"
    if src.is_file():
        shutil.copy2(src, data_dir / "prts_professions.json")
        n = len(json.loads(src.read_text(encoding="utf-8")))
        print(f"  ✓ prts_professions.json  {n} 名干员  {src.stat().st_size/1024:.0f} KB")
    else:
        print("  ⚠ 缺 prts_professions.json（新干员的职业要靠联网补）")

    # 头像：**要么全量要么全无**。--no-avatars 时连索引和缓存目录都不带，
    # 免得留下一个"只有你自己池子"的半套（别人用这份包就会缺图）。
    if with_avatars:
        for name in ("avatars.json",):
            src = data / name if (data / name).is_file() else WS / "refs" / name
            if src.is_file():
                shutil.copy2(src, data_dir / name)
        src = data / "avatars"
        if not src.is_dir():
            src = WS / "refs" / "avatars"
        if src.is_dir():
            shutil.copytree(src, data_dir / "avatars", dirs_exist_ok=True)
            print(f"  ✓ 已有头像缓存 {len(list((data_dir/'avatars').glob('*.png')))} 张（稍后补齐全量）")


    if with_avatars and (data / "avatars").is_dir():
        shutil.copytree(data / "avatars", data_dir / "avatars", dirs_exist_ok=True)
        n = len(list((data_dir / "avatars").glob("*.png")))
        print(f"  ✓ 头像缓存 {n} 张")

    cfg = {
        "box_file": box_path.name,
        "fetch_avatars": True,
        "roll_defaults": {
            "tier": 0, "mode": "quota", "n": 12, "avoid_last": 1, "exclude": "",
            "rarities": [1, 2, 3, 4, 5, 6],
            # 八个职业各一名（与 Rust 侧 roll::default_floor 保持一致）
            "quota": {"先锋": 1, "近卫": 1, "重装": 1, "狙击": 1, "术师": 1, "医疗": 1, "辅助": 1, "特种": 1},
        },
    }
    (data_dir / "config.json").write_text(
        json.dumps(cfg, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"  ✓ config.json（box_file = {box_path.name}）")
    return app_dir


def prefetch_into(data: Path) -> bool:
    """把**全游戏干员**的头像抓进数据目录（可累积：已有的不再下载）。
    用 release exe 的无界面模式 + --root 指向数据目录，这样缓存留在数据目录里，
    下次打包是增量的。返回是否达到全量覆盖。"""
    print("=== 头像全量预热（全游戏干员，断网也能有脸）===")
    log = data / "prefetch.log"
    if log.exists():
        log.unlink()

    idx = data / "prts_professions.json"
    if not idx.is_file():
        idx = WS / "refs" / "prts_professions.json"
    target = len(json.loads(idx.read_text(encoding="utf-8"))) if idx.is_file() else None

    t0 = time.time()
    subprocess.run([str(SRC_EXE), "--prefetch-avatars", "--root", str(data.resolve())], timeout=7200)
    msg = log.read_text(encoding="utf-8", errors="replace").strip() if log.exists() else "(没有日志)"

    files = list((data / "avatars").glob("*.png"))
    size = sum(f.stat().st_size for f in files) / 1024 / 1024
    print(f"  {msg}  ·  用时 {time.time()-t0:.0f}s  ·  {len(files)} 张 / {size:.1f} MB")
    if target:
        if len(files) >= target:
            print(f"  ✓ 全量覆盖（索引里 {target} 名干员）")
            return True
        print(f"  ⚠ 差 {target - len(files)} 张（索引 {target}）——多半是 wiki 上还没人传图，"
              f"这些人在界面上退化成色块")
        return False
    return len(files) > 0


def main() -> int:
    ap = argparse.ArgumentParser(description="打便携包")
    ap.add_argument("--data", default=str(ROOT / "devdata"), help="数据来源目录")
    ap.add_argument("--box", help="用哪份 box（默认取 data 下最新的 box*.json）")
    ap.add_argument("--out", default=str(WS / "dist"), help="输出目录")
    ap.add_argument("--no-avatars", action="store_true", help="不预热头像")
    ap.add_argument("--no-build", action="store_true", help="跳过编译")
    ap.add_argument("--refresh-index", action="store_true", help="打包前重抓职业索引")
    args = ap.parse_args()

    data = Path(args.data)
    out = Path(args.out)
    if not data.is_dir():
        sys.exit(f"数据目录不存在：{data}")
    if args.refresh_index:
        refresh_index(data)

    if not args.no_build:
        build()
    elif not SRC_EXE.is_file():
        sys.exit(f"没有现成产物，别加 --no-build：{SRC_EXE}")

    box_path = pick_source(data, args.box)
    print(f"=== 2/5 box：{box_path.name} ===")

    if args.no_avatars:
        print("=== 3/5 跳过头像（--no-avatars：一张都不带，用时联网取）===")
    else:
        print("=== 3/5 预热全量头像（抓进数据目录，可累积）===")
        prefetch_into(data)

    print("=== 4/5 组装便携目录 ===")
    app_dir = assemble(out, data, box_path, with_avatars=not args.no_avatars)

    print("\n=== 5/5 完成 ===")
    total = sum(f.stat().st_size for f in app_dir.rglob("*") if f.is_file())
    print(f"  {app_dir}")
    print(f"  {app_dir / (EXE_NAME + '.exe')}  {(app_dir / (EXE_NAME + '.exe')).stat().st_size/1024/1024:.1f} MB")
    print(f"  整个文件夹 {total/1024/1024:.1f} MB")
    print("\n  这个文件夹拷到哪都能跑；data\\ 与 exe 平级，配置/历史/头像都在里面。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
