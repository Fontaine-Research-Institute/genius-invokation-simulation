#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""check_no_index.py —— 全树扫描取值式下标（仅提示，不阻断构建）。

目的（用户要求 / D69）：标签与文本数据必须字段访问（TAGS.location），
不许下标取数（x[x]）。本脚本用来发现「有人偷偷写下标」。

扫描范围：crates/ 下所有 *.rs（含 tests），全树，不限于 tags/、generated/；
但排除基础库目录 crates/richtext/（D75，见下方常量处的理由）。

忽略清单：tools/ignored_index.txt，每行一条：
    <相对路径> :: <该处唯一代码片段>    # 理由（可选）
- 匹配规则：路径精确 + 该片段在命中行内精确子串出现；不用行号（不稳定）。
- 同一文件里片段重复时，请写足够特殊的片段使其唯一。
- 清单里登记了、但源码里已经找不到该片段的条目 = 已失效，报告会提醒清理。
- 片段在「该文件」里出现超过 1 次 = 片段不够唯一，该条目**不成立**：对应下标按未忽略处理，
  报告单独列「多条命中 → 忽略不成立」，其余条目继续生效、扫描不中断（D70）。

判定：找「取值式下标」——表达式后紧跟 [：
- 标识符[（含 a [i] 这种空格写法）；
- )[（如 foo()[i]）；
- ][（如 a[0][1]）。
排除：属性 #[...] / 内部属性 #![...]、类型位置（[T; N] / &[T] / Vec<[u8; 4]> /
: [T] / &'a [T] / &mut [T]）、字符串与字符字面量、注释、宏调用 name![...]。
原则：宁可少报，也别乱报。

输出：只读，结果只打 stdout（中文人读日志），不创建 / 不写入任何文件
（包括忽略清单也只读，绝不回写）。退出码恒为 0，不产生任何 GitHub 注解。

D73：文件以「共享只读」方式打开（不加锁、不独占，读取期间别人可照常编辑）；
若某文件在扫描期间被改 / 删除 / 读不出来，脚本自己捕获，在「读取失败」一节写清
（文件 + 原因），跳过该文件继续扫其余文件，最后统计单独列「读取失败 N 个」。
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
SCAN_ROOT = REPO_ROOT / "crates"
IGNORE_PATH = REPO_ROOT / "tools" / "ignored_index.txt"

# D75：这些目录是「基础库」，不按业务层的下标规则扫描。以后再有同类基础库也加到这个元组。
#   richtext 是富文本解析器（位置 / 长度 + 字节级解析），
#   用 usize / u32 以及解析器内部 x[i] 按位置切字符串是合理的。
EXCLUDED_DIRS = ("crates/richtext/",)

RAW_STRING_RE = re.compile("(?:b|c)?r(#*)" + '"')

# 这些关键字后面直接跟 [ 时是数组/类型字面量或类型位置，不是取值下标
EXCLUDE_WORDS = {
    "let", "return", "in", "match", "if", "while", "else", "yield", "move",
    "break", "continue", "loop", "await", "mut", "const", "static", "ref",
    "where", "pub", "fn", "type", "struct", "enum", "impl", "trait", "for",
    "use", "mod", "crate", "super", "as", "dyn", "async", "unsafe", "extern",
}


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


def norm_path(text: str) -> str:
    """统一路径：反斜杠转 /、去重复斜杠、去前导 ./，方便做与 OS 无关的匹配。"""
    norm = text.replace(BS, "/")
    while "//" in norm:
        norm = norm.replace("//", "/")
    while norm.startswith("./"):
        norm = norm[2:]
    return norm


def display_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return str(path)


def is_excluded_dir(display: str) -> bool:
    """是否属于 D75 排除的基础库目录。"""
    norm = "/" + norm_path(display)
    return any(("/" + norm_path(marker)) in norm for marker in EXCLUDED_DIRS)


def scan_source(text: str):
    """扫描一段源码文本，返回按行去重的命中列表 [{line, raw}]。"""
    code = strip_comments_and_strings(text)
    raw_lines = text.splitlines()
    hit_lines = set()

    for i, ch in enumerate(code):
        if ch != "[":
            continue
        p = i - 1
        while p >= 0 and code[p].isspace():
            p -= 1
        if p < 0:
            continue  # 行首（数组字面量 / 类型）
        prev = code[p]

        # 属性 #[...] / 内部属性 #![...] / 宏调用 name![...]
        if prev in "!#":
            continue

        # foo()[i] / a[0][1]：表达式后紧跟 [
        if prev in ")]":
            hit_lines.add(code.count(NL, 0, i) + 1)
            continue

        # 标识符后紧跟 [ 才可能是下标（a [i] 也认）
        if not (prev.isalnum() or prev == "_"):
            continue
        start = p
        while start >= 0 and (code[start].isalnum() or code[start] == "_"):
            start -= 1
        word = code[start + 1:p + 1]
        before = code[start] if start >= 0 else ""
        if before == "'":
            continue  # 生命周期：&'a [T]
        if word in EXCLUDE_WORDS:
            continue  # 关键字后的数组/类型字面量
        hit_lines.add(code.count(NL, 0, i) + 1)

    hits = []
    for line_no in sorted(hit_lines):
        raw = raw_lines[line_no - 1] if line_no - 1 < len(raw_lines) else ""
        hits.append({"line": line_no, "raw": raw.strip()})
    return hits


def parse_ignore_lines(lines):
    """解析清单行，返回 [{path, snippet, reason, raw, line}]。"""
    entries = []
    for lineno, line in enumerate(lines, start=1):
        raw = line.strip()
        if not raw or raw.startswith("#"):
            continue
        body, _, reason = raw.partition("#")
        body = body.strip()
        if " :: " in body:
            rel, snippet = body.split(" :: ", 1)
        elif "::" in body:
            rel, snippet = body.split("::", 1)
        else:
            continue
        rel = norm_path(rel.strip())
        snippet = snippet.strip()
        if not rel or not snippet:
            continue
        entries.append(
            {
                "path": rel,
                "snippet": snippet,
                "reason": reason.strip(),
                "raw": raw,
                "line": lineno,
            }
        )
    return entries


def load_ignores(path: Path):
    """读取忽略清单（共享只读）。返回 (条目, 读取错误原因或 None)。"""
    if not path.is_file():
        return [], None
    try:
        with open(path, "r", encoding="utf-8-sig", errors="strict") as fh:
            lines = fh.read().splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        return [], read_failure_reason(exc)
    return parse_ignore_lines(lines), None


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


def match_ignores(entries, hits, file_texts, failed_paths=()):
    """返回 (未忽略命中, 已忽略命中, 失效条目, 不成立条目)。

    D69：路径精确 + 片段在命中行内精确子串出现 = 忽略生效（不用行号）。
    D70：片段在该文件里出现 0 次 = 失效；出现 >1 次 = 不成立（那处按未忽略处理）。
    D73：读取失败的文件不参与失效判定（读不到 ≠ 条目失效），只进「读取失败」一节。
    """
    valid_entries = []
    stale = []
    invalid = []
    failed = {norm_path(p) for p in failed_paths}
    excluded_prefixes = ["/" + norm_path(marker) for marker in EXCLUDED_DIRS]

    for e in entries:
        if e["path"] in failed:
            continue
        if any(("/" + e["path"]).startswith(prefix) for prefix in excluded_prefixes):
            continue
        text = file_texts.get(e["path"])
        occurrences = []
        count = 0
        if text is not None:
            for lineno, line in enumerate(text.splitlines(), start=1):
                found = line.count(e["snippet"])
                if found:
                    count += found
                    occurrences.append({"line": lineno, "raw": line.strip()})
        if count == 0:
            stale.append(e)          # 失效：源码里已经找不到这个片段
        elif count == 1:
            valid_entries.append(e)  # 成立：片段在该文件里唯一
        else:
            invalid.append({"entry": e, "count": count, "occurrences": occurrences})

    unignored = []
    ignored = []
    for h in hits:
        key = norm_path(h["file"])
        matched = None
        for e in valid_entries:
            if e["path"] == key and e["snippet"] in h["raw"]:
                matched = e
                break
        if matched is None:
            unignored.append(h)
        else:
            ignored.append({"hit": h, "entry": matched})

    return unignored, ignored, stale, invalid


def collect():
    """返回 (文件列表, 文本表, 命中列表, 读取失败列表)。"""
    all_files = sorted(SCAN_ROOT.rglob("*.rs")) if SCAN_ROOT.is_dir() else []
    files = [f for f in all_files if not is_excluded_dir(display_path(f))]
    file_texts = {}
    hits = []
    read_failures = []
    for f in files:
        display = display_path(f)
        try:
            text = read_shared_text(f)
        except (OSError, UnicodeDecodeError) as exc:
            read_failures.append({"file": display, "reason": read_failure_reason(exc)})
            continue
        file_texts[norm_path(display)] = text
        for h in scan_source(text):
            hits.append({"file": display, "line": h["line"], "raw": h["raw"]})
    hits.sort(key=lambda h: (h["file"], h["line"]))
    return files, file_texts, hits, read_failures


def run_self_test() -> int:
    """内置自测（纯内存 fixture）：未忽略命中 / 已忽略命中 / 失效条目 / 排除项。"""
    print("=== 自测：取值式下标判定与忽略清单 ===")
    ok = True

    text = "let a = x[i];\nlet b = x[i];\nlet c = y[j];\n"
    hits = [dict(h, file="crates/definitions/src/x.rs") for h in scan_source(text)]
    file_texts = {"crates/definitions/src/x.rs": text}
    entries = parse_ignore_lines(
        [
            "crates/definitions/src/x.rs :: x[i]  # 测试：出现 2 次，不成立",
            "crates/definitions/src/x.rs :: y[j]  # 测试：唯一，成立",
            "crates/definitions/src/x.rs :: missing[z]  # 测试：0 次，失效",
            "crates/definitions/src/gone.rs :: z[0]  # 测试：文件不存在，失效",
        ]
    )
    unignored, ignored, stale, invalid = match_ignores(entries, hits, file_texts)
    print("未忽略命中：" + str(len(unignored)) + " 处（期望 2：两处 x[i]）")
    print("已忽略命中：" + str(len(ignored)) + " 处（期望 1：y[j]）")
    print("失效条目：" + str(len(stale)) + " 条（期望 2：missing[z] + gone.rs）")
    # D73：读取失败的文件不参与失效判定
    failed_paths = ["crates/definitions/src/gone.rs"]
    _, _, stale_after_failure, _ = match_ignores(entries, hits, file_texts, failed_paths)
    print("读取失败时跳过失效判定：" + str(len(stale_after_failure)) + " 条（期望 1：only missing[z]）")
    if len(stale_after_failure) != 1:
        ok = False
    print("不成立条目：" + str(len(invalid)) + " 条（期望 1：x[i] 出现 2 次）")
    if invalid:
        print(
            "  不成立条目命中次数：" + str(invalid[0]["count"]) + "（期望 2），出现行号："
            + "、".join(str(o["line"]) for o in invalid[0]["occurrences"])
        )
    print("（不成立条目不影响其它条目：y[j] 仍被忽略、扫描继续）")
    if len(unignored) != 2 or len(ignored) != 1 or len(stale) != 2 or len(invalid) != 1:
        ok = False

    positives = [
        ("标识符下标", "let a = x[i];"),
        ("右括号后下标", "let b = foo() [i];"),
        ("右方括号后下标", "let c = a[0][1];"),
    ]
    negatives = [
        ("属性", "#[derive(Clone)]\nstruct S;"),
        ("内部属性", "#![allow(dead_code)]"),
        ("切片引用类型", "let s: &[u8] = &[];"),
        ("数组类型", "let arr: [u8; 4] = [0; 4];"),
        ("嵌套数组类型", "let v: Vec<[u8; 4]> = Vec::new();"),
        ("字符串里的 [", "let t = \"x[0]\";"),
        ("注释里的 [", "// x[0]\nlet a = 1;"),
        ("宏调用", "let v = vec![1, 2];"),
        ("生命周期切片类型", "fn f<'a>(x: &'a [u8]) {}"),
        ("可变切片类型", "let p: &mut [u8] = &mut [];"),
        ("数组字面量", "let z = [1, 2];"),
        ("where 类型", "where [u8; 4]: Sized;"),
    ]
    print("应命中：")
    for name, src in positives:
        got = len(scan_source(src))
        print("  " + name + " → 命中 " + str(got) + " 处（期望 ≥1）")
        if got < 1:
            ok = False
    print("不应命中：")
    for name, src in negatives:
        got = len(scan_source(src))
        print("  " + name + " → 命中 " + str(got) + " 处（期望 0）")
        if got != 0:
            ok = False

    print("自测结果：" + ("通过" if ok else "失败（仅为提示，不影响构建）"))
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="全树扫描取值式下标（仅提示，恒定退出码 0）"
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="跑内置内存 fixture 自测后退出",
    )
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test()

    print("=== 下标访问扫描（取值式下标）===")
    print("要求：标签与文本数据必须字段访问（如 TAGS.location），不许下标取数（x[x]）。")
    print("扫描范围：crates/ 下所有 *.rs（含 tests）；排除属性、类型位置、字符串/注释、宏调用。")
    print("排除目录（D75 基础库，不扫）：" + "、".join(EXCLUDED_DIRS))
    print("")

    files, file_texts, hits, read_failures = collect()
    missing_ignore_file = not IGNORE_PATH.is_file()
    entries, ignore_error = load_ignores(IGNORE_PATH)
    if ignore_error:
        read_failures.append({"file": "tools/ignored_index.txt", "reason": ignore_error})
    failed_paths = [r["file"] for r in read_failures]
    unignored, ignored, stale, invalid = match_ignores(entries, hits, file_texts, failed_paths)

    print("总文件数：" + str(len(files)))
    print(
        "忽略清单：tools/ignored_index.txt，"
        + str(len(entries)) + " 条" + ("（文件不存在，按空清单处理）" if missing_ignore_file else "")
    )
    print("")

    print("1) 未忽略的下标命中：" + ("无" if not unignored else "共 " + str(len(unignored)) + " 处"))
    for h in unignored:
        print("  - " + h["file"] + ":" + str(h["line"]) + " / 该行：" + h["raw"])
    if unignored:
        print("")
        print("共 " + str(len(unignored)) + " 处；仅为提示，不影响构建。")
    print("")

    print("2) 已失效的忽略条目：" + ("无" if not stale else "共 " + str(len(stale)) + " 条"))
    for e in stale:
        print(
            "  - tools/ignored_index.txt:" + str(e["line"]) + " / "
            + e["path"] + " :: " + e["snippet"]
            + ("（理由：" + e["reason"] + "）" if e["reason"] else "")
        )
    if stale:
        print("  提醒：这些条目在源码里已找不到对应片段，建议从清单删除。")
    print("")

    print(
        "3) 多条命中 → 忽略不成立（片段不够唯一，对应下标按未忽略处理）："
        + ("无" if not invalid else "共 " + str(len(invalid)) + " 条")
    )
    for item in invalid:
        e = item["entry"]
        print(
            "  - tools/ignored_index.txt:" + str(e["line"]) + " / "
            + e["path"] + " :: " + e["snippet"]
            + " / 命中 " + str(item["count"]) + " 次"
        )
        for o in item["occurrences"]:
            print("      · 第 " + str(o["line"]) + " 行：" + o["raw"])
    if invalid:
        print("  提醒：请把片段写得更特殊，或改成能唯一匹配该行的片段。")
    print("")

    print("4) 读取失败：" + ("无" if not read_failures else "共 " + str(len(read_failures)) + " 个"))
    for f in read_failures:
        print("  - " + f["file"] + " / " + f["reason"])
    if read_failures:
        print("  说明：这些文件已跳过、未阻断扫描；请检查是否编辑到一半或已被删除。")
    print("")

    print("5) 统计：")
    print("   - 总文件数：" + str(len(files)))
    print("   - 命中数（全部取值式下标）：" + str(len(hits)))
    print("   - 未忽略：" + str(len(unignored)))
    print("   - 已忽略：" + str(len(ignored)))
    print("   - 忽略条目：" + str(len(entries)) + "（失效 " + str(len(stale)) + "，不成立 " + str(len(invalid)) + "）")
    print("   - 读取失败：" + str(len(read_failures)) + " 个")
    print("")
    print("结论：仅为提示，不影响构建（退出码 0）。")
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
