#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""check_bare_int.py —— 扫描裸整型与手写裸算术（仅提示，不阻断构建）。

规格：分工计划.md §8.3。
- 扫描 crates/definitions、crates/cards（具体卡牌，D240）、crates/solver、crates/app 下的 *.rs；
  目录不存在时打印「尚未出现，跳过」并正常结束。
- 只看代码（先去掉注释 / 字符串 / 字符字面量），查两类：
  1) 裸整型类型名：u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize；
  2) 手写二元 + - * /（排除解引用、glob 导入、指针类型、trait bound、一元负号）。
- 白名单：definitions/src/base/（历史写法 definitions/base/ 也认）下的 id 类型声明（整目录）；
  #[repr(u8)] 这类属性行。
  .len() 之类的 usize 返回值不在白名单（按规格必须用 Nat 或新类型）。
- 契约白名单：某行含裸整型、且行尾带 // bare-int-ok: <理由> 时，该行不计命中，
  改计入「契约白名单跳过」并逐行列出（便于审计，防止滥用）；用于引擎 API 契约
  （如 SelectionSource 的 usize）这类无法避免的地方。
- 只读：文件以共享只读方式打开（不加锁、不独占），读取期间别人可以照常编辑；
  某文件在扫描期间被改 / 删除 / 读不出来时，记入「读取失败」并跳过，继续扫其余文件（D73）。
- 输出中文人读日志，只打 stdout；退出码恒为 0：命中只提示，不影响构建。
"""

from __future__ import annotations

import argparse
import re
import sys
import traceback
from pathlib import Path

BS = chr(92)
NL = chr(10)

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TARGETS = ["crates/definitions", "crates/cards", "crates/solver", "crates/app"]

BARE_INT_RE = re.compile(r"\b(?:u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)\b")
REPR_RE = re.compile(r"#\[\s*repr\s*\(")
BARE_INT_OK_RE = re.compile(r"//\s*bare-int-ok\s*:\s*(\S[^\n]*)")
RAW_STRING_RE = re.compile("(?:b|c)?r(#*)" + '"')

# 可以作为「操作数结尾」的字符：文字、数字、下划线、右括号、右花括号、问号
OPERAND_END = set(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_)]}?"
)
# 相乘 / 相加的右侧必须以「操作数开头」的字符继续（可排除宏重复 )*、)+ 之类）
OPERAND_START = set(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_([&*!~-"
)
# 出现在这些字符之后的一元运算符不是二元运算
UNARY_CONTEXT = set("&=([{,;:+-*/%!<>?|~^")
# 这些关键字之后的一元运算符不是二元运算
UNARY_KEYWORDS = {"return", "break", "yield", "await", "else", "match", "if", "while", "in"}


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

        if ch == "/" and nxt == "/":
            while i < n and src[i] != "\n":
                out.append(" ")
                i += 1
            continue

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


# definitions 的 id 新类型声明目录：实际在 crates/definitions/src/base/，
# 早期文档写作 definitions/base/，两种都认（统一规范化后再匹配，不依赖前导斜杠）。
BASE_DIR_MARKERS = ("/definitions/src/base/", "/definitions/base/")


def is_base_file(display: str) -> bool:
    norm = "/" + norm_path(display)
    return any(marker in norm for marker in BASE_DIR_MARKERS)


def prev_non_space(code: str, i: int):
    while i >= 0 and code[i].isspace():
        i -= 1
    return i if i >= 0 else None


def next_non_space(code: str, i: int):
    n = len(code)
    while i < n and code[i].isspace():
        i += 1
    return i if i < n else None


def prev_word(code: str, i: int) -> str:
    m = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*$", code[:i])
    return m.group(1) if m else ""


def is_trait_bound(code: str, i: int) -> bool:
    """判断 pos i 的 + 是 trait bound 组合（T: A + B / impl / dyn），不是算术。"""
    stmt_start = max(
        code.rfind(";", 0, i), code.rfind("{", 0, i), code.rfind("}", 0, i)
    ) + 1
    prefix = code[stmt_start:i]
    if re.search(r"\b(?:dyn|impl)\b", prefix):
        return True
    m = re.search(r"(?<!:):(?!:)([^:;{}]*)$", prefix)
    if not m:
        return False
    seg = m.group(1).strip()
    if not seg or "=" in seg or "," in seg:
        return False
    return bool(re.match(r"(?:[A-Z_]|'|dyn\b|impl\b|&|\()", seg))


def scan_operators(code: str):
    """返回 [(offset, symbol)]：代码里手写的二元 + - * /。"""
    hits = []
    n = len(code)
    for i, ch in enumerate(code):
        if ch not in "+-*/":
            continue
        p = prev_non_space(code, i - 1)
        q = next_non_space(code, i + 1)

        if ch == "*":
            if p is None:
                continue
            if code[max(0, i - 2):i] == "::":
                continue  # glob 导入 use ...::*
            if q is not None and re.match(r"(?:const|mut)\b", code[q:]):
                continue  # 指针类型 *const T / *mut T
            if code[p] not in OPERAND_END:
                continue  # 解引用 *p / &*x
            if q is None or code[q] not in OPERAND_START:
                continue  # 宏重复 )* / 不完整表达式
            if prev_word(code, i) in UNARY_KEYWORDS:
                continue
            hits.append((i, "*"))
        elif ch == "+":
            if p is None or q is None:
                continue
            if code[p] not in OPERAND_END:
                continue
            if code[q] not in OPERAND_START:
                continue  # 宏重复 )+ / 不完整表达式
            if is_trait_bound(code, i):
                continue
            hits.append((i, "+"))
        elif ch == "-":
            if q is not None and code[q] == ">":
                continue  # 箭头 ->
            if p is None:
                continue
            if code[p] not in OPERAND_END:
                continue
            if q is None or code[q] not in OPERAND_START:
                continue  # 不完整表达式
            if prev_word(code, i) in UNARY_KEYWORDS:
                continue
            hits.append((i, "-"))
        else:  # /
            hits.append((i, "/"))

    return hits


def scan_text(text: str, display: str):
    """扫描一段源码文本。

    返回 (命中列表, 白名单跳过的裸整型数, 契约白名单跳过数, 契约白名单条目)。
    """
    code = strip_comments_and_strings(text)
    code_lines = code.splitlines()
    raw_lines = text.splitlines()

    in_base = is_base_file(display)
    hits = []
    whitelisted = 0
    contract_whitelisted = 0
    contract_entries = []
    contract_lines = set()

    for idx, code_line in enumerate(code_lines, start=1):
        raw = raw_lines[idx - 1] if idx - 1 < len(raw_lines) else ""
        repr_line = bool(REPR_RE.search(raw))
        contract = BARE_INT_OK_RE.search(raw)
        for m in BARE_INT_RE.finditer(code_line):
            if in_base or repr_line:
                whitelisted += 1
                continue
            if contract is not None:
                contract_whitelisted += 1
                if idx not in contract_lines:
                    contract_lines.add(idx)
                    contract_entries.append(
                        {
                            "file": display,
                            "line": idx,
                            "raw": raw.strip(),
                            "reason": contract.group(1).strip(),
                        }
                    )
                continue
            hits.append(
                {
                    "file": display,
                    "line": idx,
                    "kind": "裸整型 " + m.group(0),
                    "raw": raw.strip(),
                }
            )

    for offset, sym in scan_operators(code):
        line_no = code.count(NL, 0, offset) + 1
        raw = raw_lines[line_no - 1] if line_no - 1 < len(raw_lines) else ""
        hits.append(
            {
                "file": display,
                "line": line_no,
                "kind": "裸算术 " + sym,
                "raw": raw.strip(),
            }
        )

    # 同一行同类型只报一次，便于阅读
    seen = set()
    unique = []
    for h in hits:
        key = (h["line"], h["kind"])
        if key in seen:
            continue
        seen.add(key)
        unique.append(h)
    unique.sort(key=lambda h: (h["line"], h["kind"]))
    contract_entries.sort(key=lambda e: e["line"])
    return unique, whitelisted, contract_whitelisted, contract_entries


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


def format_hit(hit) -> str:
    return (
        "  - " + hit["file"] + ":" + str(hit["line"]) + " / "
        + hit["kind"] + " / 该行：" + hit["raw"]
    )


def run_self_test() -> int:
    """内置自测：验证 definitions/src/base 白名单（路径规范化后）确实生效（纯内存，不写文件）。"""
    print("=== 自测：base 白名单 / bare-int-ok 契约白名单 / 排除项 ===")
    base_text = (
        "pub struct CardId(pub Nat);\n"
        "pub const fn new(v: u32) -> CardId { CardId(Nat::new(v)) }\n"
    )
    foo_text = (
        "pub fn f(hp: u32) -> Nat {\n"
        "    let n: u32 = 0;\n"
        "    let x = hp + 1;\n"
        "    n\n"
        "}\n"
    )

    print("路径判定：")
    cases = [
        ("crates/definitions/src/base/id.rs", True),
        ("crates" + BS + "definitions" + BS + "src" + BS + "base" + BS + "tag.rs", True),
        ("crates/definitions/base/id.rs", True),
        ("./crates/definitions/src/base/id.rs", True),
        ("C:/tmp/work/definitions/src/base/id.rs", True),
        ("crates/definitions/src/lib.rs", False),
        ("crates/definitions/src/base_like.rs", False),
    ]
    paths_ok = True
    for display, expected in cases:
        got = is_base_file(display)
        print(
            "  " + norm_path(display) + " → " + ("白名单" if got else "不豁免")
            + "（期望 " + ("白名单" if expected else "不豁免") + "）"
        )
        if got != expected:
            paths_ok = False

    base_hits, base_whitelisted, base_contract, _ = scan_text(
        base_text, "crates/definitions/src/base/id.rs"
    )
    foo_hits, foo_whitelisted, foo_contract, foo_contract_entries = scan_text(
        foo_text, "crates/definitions/src/foo.rs"
    )
    bare_hits = [h for h in foo_hits if h["kind"].startswith("裸整型")]
    arith_hits = [h for h in foo_hits if h["kind"].startswith("裸算术")]

    # 契约白名单：带行尾标记不计命中；不带的照常命中
    marked_text = (
        "pub fn len(&self) -> usize { 0 } // bare-int-ok: 引擎 SelectionSource 契约\n"
    )
    unmarked_text = "pub fn len(&self) -> usize { 0 }\n"
    marked_hits, _, marked_contract, marked_entries = scan_text(
        marked_text, "crates/definitions/src/contract.rs"
    )
    unmarked_hits, _, unmarked_contract, _ = scan_text(
        unmarked_text, "crates/definitions/src/contract.rs"
    )

    print("fixture（内存）：")
    print(
        "  base/id.rs：白名单豁免 " + str(base_whitelisted) + " 处（期望 1），命中 "
        + str(len(base_hits)) + " 处（期望 0）"
    )
    print(
        "  foo.rs：白名单豁免 " + str(foo_whitelisted) + " 处（期望 0），裸整型 "
        + str(len(bare_hits)) + " 处（期望 2），裸算术 " + str(len(arith_hits)) + " 处（期望 1）"
    )
    print(
        "  契约白名单（带标记）：命中 " + str(len(marked_hits)) + " 处（期望 0），契约跳过 "
        + str(marked_contract) + " 处（期望 1，列出 " + str(len(marked_entries)) + " 行）"
    )
    print(
        "  契约白名单（不带标记）：命中 " + str(len(unmarked_hits)) + " 处（期望 1），契约跳过 "
        + str(unmarked_contract) + " 处（期望 0）"
    )
    fixtures_ok = (
        base_whitelisted == 1
        and not base_hits
        and foo_whitelisted == 0
        and foo_contract == 0
        and not foo_contract_entries
        and len(bare_hits) == 2
        and len(arith_hits) == 1
        and not marked_hits
        and marked_contract == 1
        and len(marked_entries) == 1
        and len(unmarked_hits) == 1
        and unmarked_contract == 0
    )
    ok = paths_ok and fixtures_ok
    print("自测结果：" + ("通过" if ok else "失败（仅为提示，不影响构建）"))
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="扫描裸整型 / 裸算术（仅提示，恒定退出码 0）"
    )
    parser.add_argument(
        "targets",
        nargs="*",
        help="可选：要扫描的目录（默认 crates/definitions crates/cards crates/solver crates/app）",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="跑内置 fixture 自测（base 白名单 / bare-int-ok 契约白名单 / 排除项）后退出",
    )
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test()

    if args.targets:
        targets = []
        for t in args.targets:
            p = Path(t)
            targets.append((t, p if p.is_absolute() else REPO_ROOT / t))
    else:
        targets = [(t, REPO_ROOT / t) for t in DEFAULT_TARGETS]

    print("=== 裸整型 / 裸算术扫描（仅提示，不阻断）===")
    print("规格：分工计划 §8.3 —— definitions 等上层 crate 禁止裸整型与手写 + - * /；")
    print("      数值一律用 Nat，算术一律用 nadd! / nsub! / nmul! / ndiv!。")
    print("")
    print("扫描目标：")

    existing = []
    for name, path in targets:
        if path.is_dir():
            existing.append((name, path))
            print("  - " + name + "：存在")
        else:
            print("  - " + name + "：目录不存在，尚未出现，跳过")

    if not existing:
        print("")
        print("结论：所有目标目录均尚未出现（当前只有 engine），无需扫描；仅为提示，不影响构建。")
        return 0

    all_hits = []
    whitelisted_total = 0
    contract_total = 0
    contract_entries = []
    read_failures = []
    print("")
    for name, path in existing:
        files = sorted(path.rglob("*.rs"))
        print("扫描范围：" + name + " 下 " + str(len(files)) + " 个 .rs 文件")
        for f in files:
            display = display_path(f)
            try:
                text = read_shared_text(f)
            except (OSError, UnicodeDecodeError) as exc:
                read_failures.append({"file": display, "reason": read_failure_reason(exc)})
                continue
            hits, whitelisted, contract_count, contract_lines = scan_text(text, display)
            all_hits.extend(hits)
            whitelisted_total += whitelisted
            contract_total += contract_count
            contract_entries.extend(contract_lines)

    bare = [h for h in all_hits if h["kind"].startswith("裸整型")]
    arith = [h for h in all_hits if h["kind"].startswith("裸算术")]

    print("")
    print(
        "白名单跳过：" + str(whitelisted_total)
        + " 处裸整型（definitions/base 下 id 声明 / #[repr(...)] 属性行）"
    )
    print("")
    print(
        "契约白名单跳过：" + ("无" if not contract_entries else "共 " + str(contract_total) + " 处（"
        + str(len(contract_entries)) + " 行，行尾 // bare-int-ok: <理由>）")
    )
    for e in contract_entries:
        print(
            "  - " + e["file"] + ":" + str(e["line"]) + " / 理由：" + e["reason"]
            + " / 该行：" + e["raw"]
        )
    print("")
    print("裸整型：" + ("无" if not bare else "共 " + str(len(bare)) + " 处"))
    for h in bare:
        print(format_hit(h))
    print("")
    print("裸算术：" + ("无" if not arith else "共 " + str(len(arith)) + " 处"))
    for h in arith:
        print(format_hit(h))
    print("")
    print("读取失败：" + ("无" if not read_failures else "共 " + str(len(read_failures)) + " 个"))
    for f in read_failures:
        print("  - " + f["file"] + " / " + f["reason"])
    if read_failures:
        print("  说明：这些文件已跳过、未阻断扫描；请检查是否编辑到一半或已被删除。")
    print("")
    if all_hits:
        print("共 " + str(len(all_hits)) + " 处；仅为提示，不影响构建。")
    else:
        print("未发现裸整型 / 裸算术；仅为提示，不影响构建。")
    print(
        "统计：契约白名单跳过 " + str(contract_total) + " 处；读取失败 "
        + str(len(read_failures)) + " 个；退出码恒 0（仅提示）。"
    )
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
