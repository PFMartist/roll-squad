"""把干员头像转成前端内嵌资源（安卓包靠它做到"运行中零联网"）。

桌面版是「exe + 平级 data\\avatars\\」（package.py 打包前预热 460 张 PNG），
安卓没有"exe 同级目录"、APK 里也不能当数据目录用，所以安卓端过去是**运行中按需联网**取图：
每个新干员 1 次 api.php + 1 张图，没批量也没限速。这里改成：同一批头像转成 WebP 放进
web\\assets\\avatars\\（= frontendDist，会被编进 APK 的 assets），前端优先用包内资源，
取不到（发布后新出的干员）才回退到后端/网络。

  · 源：app\\devdata\\avatars\\*.png（仓库里跟踪的那 460 张，本来就来自 PRTS）
  · 出：web\\assets\\avatars\\*.webp + manifest.json
  · **这一步纯本地转码，不联网** —— 所以对 wiki 是零请求
  · 幂等：webp 比 png 新就跳过；--clean 删掉生成物

用法：
    python app/make_avatar_assets.py             # 转码 + 写 manifest
    python app/make_avatar_assets.py --quality 80
    python app/make_avatar_assets.py --require    # CI 用：一张都没生成就退出码 1
    python app/make_avatar_assets.py --clean      # 删掉 web\\assets\\avatars\\
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

# 输出被管道捕获时 Windows Python 会用 GBK，中文/符号直接崩在 print 上（与 package.py 同款处理）
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

ROOT = Path(__file__).resolve().parent          # app/
WS = ROOT.parent                                # 仓库根
SRC = ROOT / "devdata" / "avatars"              # 桌面版打包用的 PNG（也是本脚本的源）
DEST = WS / "web" / "assets" / "avatars"        # 前端资源目录（随 frontendDist 编进 exe / APK）
INDEX = ROOT / "devdata" / "prts_professions.json"


def sanitize(name: str) -> str:
    """与 Rust 侧 paths.rs::avatar_file 同一条规则：Windows 非法字符换成 _。

    两边必须一致，否则前端算出来的文件名会和磁盘上的对不上。"""
    return "".join("_" if c in '\\/:*?"<>|' else c for c in name)


def convert(png: Path, quality: int, force: bool) -> tuple[bool, int, int]:
    """返回 (是否新转, 源大小, 产物大小)。"""
    from PIL import Image

    out = DEST / (png.stem + ".webp")
    if not force and out.is_file() and out.stat().st_mtime >= png.stat().st_mtime:
        return False, png.stat().st_size, out.stat().st_size

    with Image.open(png) as im:
        im.load()
        if im.mode not in ("RGBA", "LA", "P"):
            im = im.convert("RGBA")
        # exact=True：保留透明区域的 RGB，避免缩放/合成时出现黑边光晕
        # method=4：实测与 6 的体积几乎一样（5.5 vs 5.4 MB），但快 20 倍
        im.save(out, "WEBP", quality=quality, method=4, exact=True)
    return True, png.stat().st_size, out.stat().st_size


def main() -> int:
    ap = argparse.ArgumentParser(description="生成内嵌头像资源（WebP + manifest）")
    ap.add_argument("--quality", type=int, default=85, help="WebP 质量（默认 85）")
    ap.add_argument("--force", action="store_true", help="全部重转（忽略时间戳）")
    ap.add_argument("--clean", action="store_true", help="删掉 web/assets/avatars/ 后退出")
    ap.add_argument("--require", action="store_true", help="一张都没生成时退出码 1（CI 用）")
    args = ap.parse_args()

    if args.clean:
        if DEST.is_dir():
            shutil.rmtree(DEST)
            print(f"已删除 {DEST}")
        else:
            print(f"本来就没有 {DEST}")
        return 0

    if not SRC.is_dir():
        print(f"⚠ 没有源目录 {SRC}（桌面包的 data\\avatars）—— 跳过，安卓端将退回联网取图")
        return 1 if args.require else 0

    pngs = sorted(SRC.glob("*.png"))
    if not pngs:
        print(f"⚠ {SRC} 里没有 PNG —— 跳过")
        return 1 if args.require else 0

    DEST.mkdir(parents=True, exist_ok=True)
    fresh = 0
    src_bytes = out_bytes = 0
    names: list[str] = []
    for i, png in enumerate(pngs, 1):
        did, sb, ob = convert(png, args.quality, args.force)
        fresh += 1 if did else 0
        src_bytes += sb
        out_bytes += ob
        names.append(png.stem)
        if i % 100 == 0:
            print(f"  … {i}/{len(pngs)}", flush=True)

    manifest = {
        "_comment": "由 app/make_avatar_assets.py 生成：包内头像清单（名字已按 paths.rs::avatar_file 规则换成文件名）",
        "count": len(names),
        "names": sorted(names),
    }
    (DEST / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")

    print(f"=== 头像内嵌资源 ===")
    print(f"  源    {len(pngs)} 张 / {src_bytes/1048576:.1f} MB（{SRC}）")
    print(f"  产物  {len(names)} 张 / {out_bytes/1048576:.1f} MB（{DEST}，本次新转 {fresh} 张）")
    print(f"  压缩  {out_bytes/src_bytes*100:.0f}%  ·  平均 {out_bytes/len(names)/1024:.1f} KB/张"
          f"  ·  质量 {args.quality}")

    if INDEX.is_file():
        want = json.loads(INDEX.read_text(encoding="utf-8"))
        have = {sanitize(n) for n in names}
        missing = [n for n in want if sanitize(n) not in have]
        print(f"  覆盖  职业索引 {len(want)} 人 → 缺 {len(missing)} 张"
              + (f"（{'、'.join(missing[:6])}{'…' if len(missing) > 6 else ''}）" if missing else "（全量）"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
