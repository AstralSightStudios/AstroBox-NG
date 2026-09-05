#!/usr/bin/env python3
"""把 MiSans 可变字体按 unicode-range 切片。

## 为什么要切

全局字体栈是 `-apple-system, "SF Pro", "PingFang SC", "Geist", Inter, MiSans`，
字体回退是**逐字形**的：iOS / macOS 上汉字命中系统的 PingFang SC，MiSans 一次都
用不到；Android 上前面几个全部落空（PingFang 是苹果独有的），Geist 又只有拉丁
字母，于是**每一个汉字都落到 MiSans**。那是一个 11.87 MB 的 woff2，冷启动要整个
解压建表才能画出第一个汉字，`font-display: swap` 还会让全部文字先用兜底字体画
一遍再回流重排。

切片之后字形完全不变，只是浏览器只会去解析「屏幕上真的用到了那一片里的字」的
那几片。

## 怎么排序

按码位顺序切是没用的：随便一屏中文就会散落在几十片里，等于还是要全部加载。
这里按**本项目自己的用字频率**排：扫一遍 web/src 下所有文本（i18n 的 18 个语言
包、源码里的硬编码字符串、CSS 里的 content），统计字频降序排在最前面。

实测 UI 全部文案只用到 29414 个码位里的 2576 个（8.8%），所以只要把最前面
HEAD_CODEPOINTS 个码位切出来，应用自己的界面文字就一个字都不会落到兜底上。

## 为什么剩下的不切，直接复用原整包

woff2 的压缩是跨字形共享结构的，一拆开这个上下文就没了：实测无论切多大，
子集都是 ~640 B/字，而整包是 423 B/字——也就是说全切一遍，磁盘占用会从
11.87 MB 涨到 18.8 MB，安装包白白大 7 MB，而且这个开销跟片大小无关，调粒度
救不回来。

所以只切头部，尾部（生僻字，只有用户内容里才可能出现）直接挂原来那个整包，
unicode-range 设成头部的补集。这样：
- 界面文字永远只解析头部那 ~1.4 MB，整包在启动路径上一次都不会被碰到；
- 磁盘只多了头部那几片，安装包基本不变；
- 覆盖率和以前完全一样，没有任何字形会掉到系统字体上去。

## 怎么跑

    python3 -m venv .venv && .venv/bin/pip install fonttools brotli
    .venv/bin/python scripts/split-misans.py

产物（web/src/fonts/misans/）是**提交进仓库**的，正常构建不需要装 fonttools；
只有换字体或改切片策略时才需要重跑这个脚本。
"""

from __future__ import annotations

import collections
import io
import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTFont

REPO = pathlib.Path(__file__).resolve().parent.parent
SOURCE_FONT = REPO / "web/src/fonts/MiSans-VariableFont.ttf"
SCAN_ROOT = REPO / "web/src"
OUT_DIR = REPO / "web/src/fonts/misans"

FAMILY = "MiSans"
# 切成子集的码位数，取「UI 全部用字（实测 2576）」往上取整留些余量。
HEAD_CODEPOINTS = 3000
# 每片的码位数。小了片数多、CSS 里的 unicode-range 列表长；大了单片解析成本高。
# 600 大致落在「一屏中文命中 1~3 片」。
SLICE_SIZE = 600
# 兜底用的原整包，相对 OUT_DIR 的路径。
FALLBACK_FONT = "../MiSans-VariableFont.woff2"
SCAN_SUFFIXES = {".json", ".ts", ".tsx", ".css", ".md", ".html"}
# 这些永远放进第 0 片：ASCII 与常用标点，兜底路径上不该再多触发一次字体加载。
ALWAYS_FIRST = list(range(0x20, 0x7F)) + [
    0xA0, 0xB7, 0x2010, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D,
    0x2026, 0x2030, 0x2032, 0x2033, 0x203B, 0x20AC, 0x2103, 0x00D7,
]


def scan_ui_codepoints(font_cps: set[int]) -> list[int]:
    """按项目自己的用字频率降序返回码位。"""
    freq: collections.Counter[str] = collections.Counter()
    for path in SCAN_ROOT.rglob("*"):
        if not path.is_file() or path.suffix not in SCAN_SUFFIXES:
            continue
        if "fonts" in path.parts:
            continue
        try:
            freq.update(path.read_text(encoding="utf-8"))
        except (UnicodeDecodeError, OSError):
            continue
    return [ord(ch) for ch, _ in freq.most_common() if ord(ch) in font_cps]


def build_order(font_cps: set[int]) -> list[int]:
    order: list[int] = []
    seen: set[int] = set()

    def push(cp: int) -> None:
        if cp in font_cps and cp not in seen:
            seen.add(cp)
            order.append(cp)

    for cp in ALWAYS_FIRST:
        push(cp)
    for cp in scan_ui_codepoints(font_cps):
        push(cp)
    # 剩下的按码位补齐，用户内容里才会用到。
    for cp in sorted(font_cps):
        push(cp)
    return order


def to_unicode_range(codepoints: list[int]) -> str:
    """把码位压成 CSS unicode-range 的紧凑写法。"""
    parts: list[str] = []
    ordered = sorted(codepoints)
    start = prev = ordered[0]
    for cp in ordered[1:]:
        if cp == prev + 1:
            prev = cp
            continue
        parts.append(f"U+{start:X}" if start == prev else f"U+{start:X}-{prev:X}")
        start = prev = cp
    parts.append(f"U+{start:X}" if start == prev else f"U+{start:X}-{prev:X}")
    return ",".join(parts)


def subset_font(raw: bytes, codepoints: list[int]) -> bytes:
    font = TTFont(io.BytesIO(raw))
    options = subset.Options()
    options.flavor = "woff2"
    # 可变字体：fvar / gvar / STAT 必须留着，App.css 用的是
    # font-weight: var(--default-font-weight, 300)，靠的就是 wght 轴。
    options.retain_gids = False
    options.desubroutinize = False
    options.hinting = True
    options.legacy_kern = False
    options.notdef_outline = False
    options.recalc_bounds = False
    options.drop_tables += ["DSIG"]
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=codepoints)
    subsetter.subset(font)
    buf = io.BytesIO()
    font.save(buf)
    font.close()
    return buf.getvalue()


def main() -> int:
    if not SOURCE_FONT.exists():
        print(f"找不到源字体：{SOURCE_FONT}", file=sys.stderr)
        return 1

    raw = SOURCE_FONT.read_bytes()
    font_cps = set(TTFont(io.BytesIO(raw), lazy=True).getBestCmap())
    order = build_order(font_cps)
    head, tail = order[:HEAD_CODEPOINTS], order[HEAD_CODEPOINTS:]
    slices = [head[i : i + SLICE_SIZE] for i in range(0, len(head), SLICE_SIZE)]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for stale in OUT_DIR.glob("*.woff2"):
        stale.unlink()

    def face(src: str, codepoints: list[int]) -> str:
        return (
            # 描述符刻意与 App.css 里原来那条**逐字一致**（只多一个
            # unicode-range），这样切片前后浏览器的字体匹配行为完全相同。
            # 注意原来就没声明 font-weight 范围——可变字重因此可能没真正生效，
            # 但那是另一码事，不在这次改动里顺手改。
            "@font-face {\n"
            f'    font-family: "{FAMILY}";\n'
            "    font-display: swap;\n"
            f'    src: url("{src}") format("woff2");\n'
            f"    unicode-range: {to_unicode_range(codepoints)};\n"
            "}\n"
        )

    faces: list[str] = []
    total = 0
    for index, codepoints in enumerate(slices):
        data = subset_font(raw, codepoints)
        name = f"MiSans-{index:03d}.woff2"
        (OUT_DIR / name).write_bytes(data)
        total += len(data)
        faces.append(face(f"./{name}", codepoints))
        print(
            f"  [{index}] {name}  {len(codepoints):4d} 码位  {len(data) / 1024:7.1f} kB"
        )

    # 尾部不切，直接指回原整包（见文件头说明）。
    faces.append(face(FALLBACK_FONT, tail))

    header = (
        "/* 由 scripts/split-misans.py 生成，不要手改。\n"
        " *\n"
        " * MiSans 整包 11.87 MB，而在全局字体栈里它是 Android 上唯一能出汉字的字体\n"
        " * （iOS / macOS 命中系统的 PingFang SC，一次都走不到这里）。不切的话，\n"
        " * 冷启动要把整包解压建表才能画出第一个汉字。\n"
        " *\n"
        " * 头部按本项目自己的用字频率切片，覆盖界面全部文案；尾部（生僻字，只有\n"
        " * 用户内容里才可能出现）直接指回原整包，因此磁盘占用几乎没变。\n"
        f" * 头部 {len(slices)} 片 / {len(head)} 个码位，尾部 {len(tail)} 个码位。\n"
        " */\n\n"
    )
    (OUT_DIR / "misans.css").write_text(header + "\n".join(faces), encoding="utf-8")

    print(
        f"\n头部 {len(slices)} 片合计 {total / 1024 / 1024:.2f} MB"
        f"，尾部 {len(tail)} 个码位复用原整包"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
