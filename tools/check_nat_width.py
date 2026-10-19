#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""check_nat_width.py —— 扫描非 32 位 Nat 用法（仅提示，不阻断构建）。

规格：分工计划.md §8.3、决策日志 D37。
- 默认一律用 Nat（= Nat32）；故意用 Nat8 / Nat16 / Nat64 / Nat128，
  必须登记到 git 之外的不入库/nat-宽度统计.md，并在该行写
  // nat-width-ok: <理由> 作为本扫描认可的登记标记。
- 递归扫描 crates/ 下所有 *.rs，跳过 crates/engine/src/nat.rs
  （那里是一族宽度的定义与自身测试）。
- 只统计「代码里的用法」：注释、字符串、字符字面量里的提及不算用法，
  会作为补充信息单独列出，方便和统计表核对。
- 只读：结果只打到 stdout（中文人读日志），不创建 / 不写入任何文件。
- 退出码恒为 0：命中只提示，不影响构建。
"""

from __future__ import annotations

import argparse
import re
import sys
import traceback
from pathlib import Path

BS = chr(92)

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCAN_ROOT = REPO_ROOT / "crates"
SKIP_REL = "engine/src/nat.rs"

# D75：排除基础库目录（以后再有同类基础库也加到这个元组）。
#   richtext 是富文本解析器（位置 / 长度 + 字节级解析），
#   用 usize / u32、内部 x[i] 按位置切字符串是合理的，不属于 Nat 宽度的业务层。
EXCLUDED_DIRS = ("crates/richtext/",)

WIDTH_RE = re.compile(r"\bNat(?:8|16|64|128)\b")
MARKER_RE = re.compile(r"//\s*nat-width-ok\s*:\s*(\S[^\n]*)")
RAW_STRING_RE = re.compile("(?:b|c)?r(#*)" + '"')


def _blank(ch: str) -> str:
    return "\n" if ch == "\n" else " "


def _char_literal_end(src: str, i: int):
    """如果 src[i] 是字符字面量的开头，返回结束位置（不含）；否则返回 None。"""
    n = len(src)
    j = i + 1
    if j >= n:
        return None
    if src[j] == BS:
        k = j + 1
        if k < n and src[k] == "u" and k + 1 < n and src[k + 1] == "{":
            k += 2
            while k < n and src[k] != "}":
                k += 1
            k += 1
        else:
            k += 1
        if k < n and src[k] == "'":
            return k + 1
        return None
    if src[j] == "'":
        return None
    if j + 1 < n and src[j + 1] == "'":
        return j + 2
    return None


def strip_comments_and_strings(src: str) -> str:
    """把注释 / 字符串 / 字符字面量换成空格，保留换行与字符数（行号不变）。"""
    out = []
    i = 0
    n = len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""

        # 行注释
        if ch == "/" and nxt == "/":
            while i < n and src[i] != "\n":
                out.append(" ")
                i += 1
            continue

        # 块注释（Rust 允许嵌套）
        if ch == "/" and nxt == "*":
            depth = 1
            out.append("  ")
            i += 2
            while i < n and depth > 0:
                if src[i] == "/" and i + 1 < n and src[i + 1] == "*":
                    depth += 1
                    out.append("  ")
                    i += 2
                elif src[i] == "*" and i + 1 < n and src[i + 1] == "/":
                    depth -= 1
                    out.append("  ")
                    i += 2
                else:
                    out.append(_blank(src[i]))
                    i += 1
            continue

        # 原始字符串 r"..." / r#"..."# / br"..." / cr#"..."#
        if ch in "rbc":
            m = RAW_STRING_RE.match(src, i)
            if m:
                hashes = m.group(1)
                start = i + m.end()
                closing = '"' + hashes
                end = src.find(closing, start)
                after = n if end < 0 else end + len(closing)
                for c in src[i:after]:
                    out.append(_blank(c))
                i = after
                continue

        # 普通字符串
        if ch == '"':
            out.append(" ")
            i += 1
            while i < n:
                c = src[i]
                if c == BS and i + 1 < n:
                    out.append(" ")
                    out.append(_blank(src[i + 1]))
                    i += 2
                    continue
                if c == "\n":
                    out.append("\n")
                    i += 1
                    break
                if c == '"':
                    out.append(" ")
                    i += 1
                    break
                out.append(" ")
                i += 1
            continue

        # 字符字面量 vs 生命周期（'a / 'static 要保留）
        if ch == "'":
            end = _char_literal_end(src, i)
            if end is not None:
                for c in src[i:end]:
                    out.append(_blank(c))
                i = end
                continue
            out.append(ch)
            i += 1
            continue

        out.append(ch)
        i += 1
    return "".join(out)


def display_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return str(path)


def norm_path(text: str) -> str:
    """统一路径：反斜杠转 /、去重复斜杠、去前导 ./，方便做与 OS 无关的匹配。"""
    norm = text.replace(BS, "/")
    while "//" in norm:
        norm = norm.replace("//", "/")
    while norm.startswith("./"):
        norm = norm[2:]
    return norm


def is_nat_definition_file(display: str, rel: str) -> bool:
    """是否要跳过 engine/src/nat.rs（统一规范化后匹配，兼容不同 --root 写法）。"""
    if norm_path(rel) == SKIP_REL:
        return True
    disp = "/" + norm_path(display)
    return disp.endswith("/crates/engine/src/nat.rs")


def is_excluded_dir(display: str) -> bool:
    """是否属于 D75 排除的基础库目录。"""
    norm = "/" + norm_path(display)
    return any(("/" + norm_path(marker)) in norm for marker in EXCLUDED_DIRS)


def read_shared_text(path: Path) -> str:
    """以共享只读方式打开并读取（不加锁、不独占），读取期间别人可以照常编辑。"""
    with open(path, "r", encoding="utf-8-sig", errors="strict") as fh:
        return fh.read()


def read_failure_reason(exc: Exception) -> str:
    if isinstance(exc, UnicodeDecodeError):
        return "读取失败：内容不是有效 UTF-8（可能在扫描期间被改写）"
    return (
        "读取失败：文件在扫描期间被修改 / 删除（"
        + type(exc).__name__ + "：" + str(exc) + "）"
    )


def collect(scan_root: Path):
    """返回 (扫描文件列表, 命中列表, 注释提及列表, 跳过列表, 读取失败列表)。"""
    scanned = []
    rows = []
    mentions = []
    skipped = []
    read_failures = []

    for path in sorted(scan_root.rglob("*.rs")):
        try:
            rel = path.relative_to(scan_root).as_posix()
        except ValueError:
            rel = path.as_posix()
        name = display_path(path)
        if is_excluded_dir(name):
            continue
        if is_nat_definition_file(name, rel):
            skipped.append(norm_path(rel))
            continue

        scanned.append(name)
        try:
            text = read_shared_text(path)
        except (OSError, UnicodeDecodeError) as exc:
            read_failures.append({"file": name, "reason": read_failure_reason(exc)})
            continue

        code_lines = strip_comments_and_strings(text).splitlines()
        raw_lines = text.splitlines()

        file_rows = []
        row_index = {}
        for lineno, raw in enumerate(raw_lines, start=1):
            code = code_lines[lineno - 1] if lineno - 1 < len(code_lines) else ""
            code_kinds = [m.group(0) for m in WIDTH_RE.finditer(code)]
            for kind in code_kinds:
                key = (lineno, kind)
                row = row_index.get(key)
                if row is None:
                    row = {
                        "file": name,
                        "line": lineno,
                        "kind": kind,
                        "marked": bool(MARKER_RE.search(raw)),
                        "raw": raw.strip(),
                        "count": 1,
                    }
                    row_index[key] = row
                    file_rows.append(row)
                else:
                    row["count"] += 1
            raw_kinds = [m.group(0) for m in WIDTH_RE.finditer(raw)]
            if len(raw_kinds) > len(code_kinds):
                mention_kinds = []
                for kind in raw_kinds:
                    if kind not in mention_kinds:
                        mention_kinds.append(kind)
                mentions.append(
                    {
                        "file": name,
                        "line": lineno,
                        "kinds": mention_kinds,
                        "raw": raw.strip(),
                    }
                )
        rows.extend(file_rows)

    return scanned, rows, mentions, skipped, read_failures


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="扫描非 32 位 Nat 用法（仅提示，恒定退出码 0）"
    )
    parser.add_argument(
        "--root",
        default=str(DEFAULT_SCAN_ROOT),
        help="扫描根目录（默认：仓库根的 crates/）",
    )
    args = parser.parse_args(argv)
    scan_root = Path(args.root)

    print("=== Nat 宽度扫描（非 32 位 Nat 用法）===")
    print("规格：分工计划 §8.3 / 决策日志 D37 —— 默认一律用 Nat（= Nat32）；")
    print("      故意用其它宽度必须登记，并在该行写 // nat-width-ok: <理由>。")
    print("")

    if not scan_root.is_dir():
        print("扫描范围：未找到目录 " + str(scan_root) + "，跳过扫描。")
        print("结论：仅为提示，不影响构建（退出码 0）。")
        return 0

    scanned, rows, mentions, skipped, read_failures = collect(scan_root)
    skipped_text = "、".join(skipped) if skipped else "无"
    print(
        "扫描范围：" + display_path(scan_root) + " 下 " + str(len(scanned))
        + " 个 .rs 文件（已跳过 " + skipped_text + "）"
    )
    print("排除目录（D75 基础库，不扫）：" + "、".join(EXCLUDED_DIRS))
    print("")

    if not rows:
        print("非 32 位 Nat 用法：无")
    else:
        unmarked_occ = sum(r["count"] for r in rows if not r["marked"])
        total_occ = sum(r["count"] for r in rows)
        print(
            "非 32 位 Nat 用法：共 " + str(total_occ) + " 处（未标记 "
            + str(unmarked_occ) + " 处，已标记 " + str(total_occ - unmarked_occ) + " 处）"
        )
        print("")
        for r in rows:
            marked = "已标记" if r["marked"] else "未标记"
            kind = r["kind"] + ("（" + str(r["count"]) + " 次）" if r["count"] > 1 else "")
            print(
                "  - " + r["file"] + ":" + str(r["line"]) + " / " + kind
                + " / " + marked + " / 该行：" + r["raw"]
            )
        print("")
        print("共 " + str(total_occ) + " 处；仅为提示，不影响构建。")

    if mentions:
        print("")
        print("补充：注释 / 文档里的提及（不算用法，仅供参考）：")
        for m in mentions:
            print(
                "  - " + m["file"] + ":" + str(m["line"]) + " / "
                + "、".join(m["kinds"]) + " / 该行：" + m["raw"]
            )

    print("")
    print("读取失败：" + ("无" if not read_failures else "共 " + str(len(read_failures)) + " 个"))
    for f in read_failures:
        print("  - " + f["file"] + " / " + f["reason"])
    if read_failures:
        print("  说明：这些文件已跳过、未阻断扫描；请检查是否编辑到一半或已被删除。")
    print("")
    print("统计：读取失败 " + str(len(read_failures)) + " 个；仅为提示，不影响构建（退出码 0）。")
    return 0


def _force_utf8_stdio() -> None:
    """Windows runner 的 Python 默认可能是 cp1252：强制 UTF-8，避免中文日志抛 UnicodeEncodeError。"""
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass


if __name__ == "__main__":
    _force_utf8_stdio()
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except Exception:
        print("扫描脚本内部错误（仅为提示，不影响构建）：")
        traceback.print_exc()
        raise SystemExit(0)
