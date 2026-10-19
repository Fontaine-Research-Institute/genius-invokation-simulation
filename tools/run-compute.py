#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""按「计算名」运行仓库里的 Rust 计算程序（D231 - D236）。

用法（在 new/ 目录下跑；脚本会自己定位仓库根，所以在别处跑也行）：

    python tools/run-compute.py <计算名>
    python tools/run-compute.py --all
    python tools/run-compute.py --list
    python tools/run-compute.py --configs

一个「计算」= crates/compute/src/bin/<计算名>/ 一个文件夹：
  - 入口固定是文件夹里的 main.rs；
  - 同目录下其它 .rs 用 'mod xxx;' 当子模块
    （Cargo 自动把该文件夹识别为名为 <计算名> 的 bin target）；
  - 文件夹名就是计算名，只用小写字母 / 数字 / - / _；
  - 可选配置文件 compute.json（用例列表 / 超时）。

本脚本**自己不接受任何计算参数**（命令行只有一个计算名）：
某个计算要跑哪些参数组合，全部定义在它目录的 compute.json 的 cases 里，
脚本只负责把用例的 cmd 拼在 cargo run 的 '--' 之后，不解释语义。

compute.json（可选，放在计算目录里）：

    {
      "cases": [
        { "name": "默认", "cmd": [], "expected_file": "expected/default.json" },
        { "name": "大副本案", "cmd": ["--big"] },
        { "name": "第二个参数集", "cmd": ["--k", "7"],
          "expected_file": "expected/k7.json" }
      ],
      "timeout_minutes": 10
    }

用例字段：
  name             用例名（可选，默认「用例N」），只用于日志 / 汇总；
  cmd              该用例的执行参数（字符串数组，可选，默认 []）；
                   原样拼在 cargo run ... -- 之后传给计算的 bin；
  expected_file    正确答案文件的相对路径（可选；**相对该计算目录**，可带子目录，
                   如 expected/default.json，用 / 分隔）；
                   没有这个字段 = 该用例「无正确答案」，只运行、输出、视作绿；
                   文件不存在 = 打警告、按「无答案」处理，并在汇总里标出警告；
  timeout_minutes  该用例的「编译+运行」超时（可选，缺省用全局值）。

**正确答案只放文件、不写进配置内容（D236）**：配置里只给 expected_file 地址，
比对时把程序 stdout 解析成 JSON 与答案文件内容深度比较。

运行行为：
  - 同一计算内的多条用例**串行**执行（一条接一条，不并发）；
  - 某条用例失败后继续跑完剩余用例，记录每条结果，最后打印汇总；
  - 有正确答案文件：运行 -> 解析 stdout 为 JSON -> 与答案文件深度比较；
  - 没有 expected_file / 答案文件缺失：只运行并输出结果，视作成功；
  - 超时 / 编译失败 / 运行非零退出：该用例失败；
  - 所有用例都通过，退出码才是 0。

退出码：
  0    全部用例通过（含「只运行、无正确答案」的视作成功）；
  1    计算名非法 / 不存在 / 配置非法；--all 有红项时也返回 1；
  3    stdout 不是合法 JSON，或与答案文件不一致；
  124  超过用例 / 全局的 timeout_minutes；
  其它 第一条失败的编译 / 运行退出码原样返回（常见 101）；127 找不到 cargo。
"""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import List, Optional, Tuple

# 仓库根 = 本脚本所在目录（tools/）的上一级。
REPO_ROOT = Path(__file__).resolve().parent.parent

# 计算程序目录：crates/compute/src/bin/<计算名>/main.rs
COMPUTE_CRATE_REL = Path("crates") / "compute"
BIN_DIR_REL = COMPUTE_CRATE_REL / "src" / "bin"
BIN_DIR = REPO_ROOT / BIN_DIR_REL

# workspace 里的 package 名（cargo run -p <这个> --bin <计算名>）。
PACKAGE = "gi-genius-invokation-compute"

# 每个计算目录里的可选配置文件。
CONFIG_FILENAME = "compute.json"

# 计算名规则：小写字母或数字开头，之后允许小写字母 / 数字 / - / _。
# kebab 与 snake 都行；不允许路径成分，名字必须精确等于文件夹名。
NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9_-]*$")

# diff 摘要里「期望 / 实际」各自最多显示多少行。
SUMMARY_LINES = 40

# 退出码。
EXIT_OK = 0
EXIT_USAGE = 1
EXIT_MISMATCH = 3
EXIT_TIMEOUT = 124

USAGE = """用法：
  python tools/run-compute.py <计算名>    按用例顺序运行该计算的全部用例并汇总
  python tools/run-compute.py --all       运行所有带 compute.json 的计算
  python tools/run-compute.py --list      列出全部计算名，标出配置 / 用例 / 正确答案状态
  python tools/run-compute.py --configs   只列出带 compute.json 的计算名（每行一个）

一个计算 = crates/compute/src/bin/<计算名>/ 文件夹（入口 main.rs，可放多个 rs 子模块）。
参数组合写在计算目录的 compute.json 的 cases 里；脚本本身不接受计算参数。
"""


class ComputeCase(object):
    """compute.json 里的一条用例。"""

    def __init__(self, name, cmd, has_expected, expected, expected_file,
                 expected_file_path, expected_file_missing, warning, timeout_minutes):
        self.name = name
        self.cmd = cmd
        self.has_expected = has_expected
        self.expected = expected
        self.expected_file = expected_file
        self.expected_file_path = expected_file_path
        self.expected_file_missing = expected_file_missing
        self.warning = warning
        self.timeout_minutes = timeout_minutes


class ComputeConfig(object):
    """一个计算目录里 compute.json 解析出来的配置。"""

    def __init__(self, cases, timeout_minutes, warnings):
        self.cases = cases
        self.timeout_minutes = timeout_minutes
        self.warnings = warnings


def configure_utf8() -> None:
    """Windows 控制台 / CI 管道下强制 UTF-8，避免中文日志抛 UnicodeEncodeError。"""
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is None:
            continue
        try:
            reconfigure(encoding="utf-8")
        except (ValueError, OSError):
            pass


def compute_dir(name: str) -> Path:
    return BIN_DIR / name


def main_rs_path(name: str) -> Path:
    return compute_dir(name) / "main.rs"


def config_path(name: str) -> Path:
    return compute_dir(name) / CONFIG_FILENAME


def display_path(path: Path) -> str:
    """日志里优先显示相对仓库根的路径，短且跨机器可读。"""
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return str(path)


def list_computes() -> List[str]:
    """列出全部可用计算名（按名字排序）。

    只认「文件夹 + 里面有 main.rs」的目录；散落的单独 .rs 文件不算计算。
    """
    if not BIN_DIR.is_dir():
        return []
    names = []
    for entry in BIN_DIR.iterdir():
        if entry.is_dir() and (entry / "main.rs").is_file():
            names.append(entry.name)
    return sorted(names)


def discover_configs() -> List[str]:
    """列出所有带 compute.json 的计算名（CI 据此决定跑哪些，不硬编码名单）。"""
    names = []
    for name in list_computes():
        if config_path(name).is_file():
            names.append(name)
    return names


def validate_name(name: str) -> Optional[str]:
    """校验计算名；合法返回 None，非法返回中文原因。"""
    if not name:
        return "计算名为空"
    if "/" in name or chr(92) in name:
        return "不能包含路径分隔符（/ 或反斜杠）"
    if ".." in name:
        return "不能包含 .."
    if name.startswith("-"):
        return "不能以 - 开头（会被当成命令行选项）"
    if not NAME_PATTERN.match(name):
        return "只能用小写字母、数字、- 和 _，且首字符须为小写字母或数字"
    return None


def print_available() -> None:
    """把可用计算名列表打给 stderr（出错时用；--list 只往 stdout 打）。"""
    names = list_computes()
    if not names:
        print(
            "crates/compute/src/bin/ 下还没有任何计算"
            "（一个计算 = 一个含 main.rs 的文件夹）。",
            file=sys.stderr,
        )
        return
    print("可用的计算名（{} 下的文件夹）：".format(BIN_DIR_REL.as_posix()), file=sys.stderr)
    for name in names:
        print("  " + name, file=sys.stderr)


def _parse_timeout_minutes(value, where: str) -> Tuple[Optional[float], Optional[str]]:
    """解析可选的超时分钟数；返回 (值或 None, 错误或 None)。"""
    if value is None:
        return None, None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None, "{} 必须是正数".format(where)
    if value <= 0:
        return None, "{} 必须大于 0".format(where)
    return float(value), None


def _load_case_expected(name: str, raw_case: dict, where: str):
    """解析一条用例的正确答案文件地址。返回 (info, 错误)。

    info 里 has_expected 为真时 expected 是答案文件解析出来的 JSON 值；
    文件不存在时 missing=True、warning 有说明，按「无答案」处理。
    """
    info = {
        "has_expected": False,
        "expected": None,
        "expected_file": None,
        "expected_file_path": None,
        "missing": False,
        "warning": None,
    }

    if "expected" in raw_case:
        return None, (
            "{} 里按 D236 起不再支持内联 expected："
            "正确答案请用 expected_file 指向该计算目录内的文件".format(where)
        )

    value = raw_case.get("expected_file")
    if value is None:
        return info, None

    if not isinstance(value, str) or not value.strip():
        return None, "{} 的 expected_file 必须是非空字符串".format(where)
    if value != value.strip():
        return None, "{} 的 expected_file 前后不能有空白".format(where)
    if chr(92) in value:
        return None, "{} 的 expected_file 请用 / 分隔（跨平台），不要用反斜杠".format(where)
    if value.startswith("/") or re.match(r"^[A-Za-z]:", value):
        return None, "{} 的 expected_file 必须是相对该计算目录的路径".format(where)

    parts = [part for part in value.split("/") if part not in ("", ".")]
    if not parts:
        return None, "{} 的 expected_file 不能是空路径".format(where)
    if any(part == ".." for part in parts):
        return None, "{} 的 expected_file 不能包含 ..".format(where)

    directory = compute_dir(name)
    target = directory.joinpath(*parts)
    try:
        target.resolve().relative_to(directory.resolve())
    except ValueError:
        return None, "{} 的 expected_file 必须位于该计算目录内".format(where)

    info["expected_file"] = value
    info["expected_file_path"] = target
    if not target.is_file():
        info["missing"] = True
        info["warning"] = (
            "{} 的 expected_file 不存在：{}（按无正确答案处理，只运行，视作成功）".format(
                where, value
            )
        )
        return info, None

    try:
        info["expected"] = json.loads(target.read_text(encoding="utf-8-sig"))
    except OSError as exc:
        return None, "{} 读 expected_file 失败：{}".format(where, exc)
    except ValueError as exc:
        return None, "{} 的 expected_file 不是合法 JSON：{}".format(where, exc)
    info["has_expected"] = True
    return info, None


def load_config(name: str) -> Tuple[Optional[ComputeConfig], Optional[str]]:
    """读计算目录的 compute.json；没有配置返回 (None, None)，配置有错返回 (None, 原因)。"""
    path = config_path(name)
    if not path.is_file():
        return None, None
    display = display_path(path)
    try:
        raw_text = path.read_text(encoding="utf-8-sig")
    except OSError as exc:
        return None, "读不了 {}：{}".format(display, exc)
    try:
        data = json.loads(raw_text)
    except ValueError as exc:
        return None, "{} 不是合法 JSON：{}".format(display, exc)
    if not isinstance(data, dict):
        return None, "{} 顶层必须是 JSON 对象".format(display)

    warnings = []

    global_timeout, error = _parse_timeout_minutes(
        data.get("timeout_minutes"), "{} 的 timeout_minutes".format(display)
    )
    if error is not None:
        return None, error

    if "cases" not in data:
        return (
            None,
            "{} 缺少 cases 数组（D235 起配置是用例列表，旧的 test/expected 写法已废弃）".format(
                display
            ),
        )
    raw_cases = data["cases"]
    if not isinstance(raw_cases, list):
        return None, "{} 的 cases 必须是数组".format(display)
    if not raw_cases:
        return None, "{} 的 cases 不能为空（至少要有一条用例）".format(display)

    cases = []
    for index, raw_case in enumerate(raw_cases):
        where = "{} 的第 {} 条用例".format(display, index + 1)
        if not isinstance(raw_case, dict):
            return None, "{} 必须是 JSON 对象".format(where)

        case_name = raw_case.get("name", "用例{}".format(index + 1))
        if not isinstance(case_name, str) or not case_name.strip():
            return None, "{} 的 name 必须是非空字符串".format(where)

        cmd = raw_case.get("cmd", [])
        if not isinstance(cmd, list):
            return None, "{} 的 cmd 必须是字符串数组".format(where)
        for item in cmd:
            if not isinstance(item, str):
                return None, "{} 的 cmd 里每一项都必须是字符串".format(where)

        info, error = _load_case_expected(name, raw_case, where)
        if error is not None:
            return None, error
        if info["warning"] is not None:
            warnings.append(info["warning"])

        case_timeout, error = _parse_timeout_minutes(
            raw_case.get("timeout_minutes"), "{} 的 timeout_minutes".format(where)
        )
        if error is not None:
            return None, error

        cases.append(
            ComputeCase(
                name=case_name,
                cmd=list(cmd),
                has_expected=info["has_expected"],
                expected=info["expected"],
                expected_file=info["expected_file"],
                expected_file_path=info["expected_file_path"],
                expected_file_missing=info["missing"],
                warning=info["warning"],
                timeout_minutes=case_timeout,
            )
        )

    return ComputeConfig(cases=cases, timeout_minutes=global_timeout, warnings=warnings), None


def describe_compute(name: str) -> str:
    """--list 用的状态说明。"""
    if not config_path(name).is_file():
        return "配置:无  用例:1（有答案 0 / 无答案 1）-> 按默认用例只运行，视作成功"
    config, error = load_config(name)
    if error is not None:
        return "配置:有(无效)  -> 运行会失败：{}".format(error)
    if config is None:
        return "配置:有"
    with_answer = sum(1 for case in config.cases if case.has_expected)
    without_answer = len(config.cases) - with_answer
    missing = sum(1 for case in config.cases if case.expected_file_missing)
    parts = [
        "配置:有",
        "用例:{}（有答案 {} / 无答案 {}）".format(
            len(config.cases), with_answer, without_answer
        ),
    ]
    if missing:
        parts.append("答案文件缺失 {} 条(警告)".format(missing))
    if config.timeout_minutes is not None:
        parts.append("全局超时:{:g} 分钟".format(config.timeout_minutes))
    return "  ".join(parts)


def print_list() -> int:
    names = list_computes()
    if not names:
        print(
            "crates/compute/src/bin/ 下还没有任何计算"
            "（一个计算 = 一个含 main.rs 的文件夹）。",
            file=sys.stderr,
        )
        return EXIT_OK
    width = max(len(name) for name in names) + 2
    print("可用计算（{} 下的文件夹）：".format(BIN_DIR_REL.as_posix()))
    for name in names:
        print("  " + name.ljust(width) + describe_compute(name))
    return EXIT_OK


def print_configs() -> int:
    names = discover_configs()
    for name in names:
        print(name)
    if not names:
        print(
            "crates/compute/src/bin/ 下没有任何带 {} 的计算。".format(CONFIG_FILENAME),
            file=sys.stderr,
        )
    return EXIT_OK


def _dumps(value) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True)


def _print_truncated(text: str, max_lines: int) -> None:
    lines = text.splitlines()
    for line in lines[:max_lines]:
        print("  " + line, file=sys.stderr)
    if len(lines) > max_lines:
        print("  ...（还有 {} 行未显示）".format(len(lines) - max_lines), file=sys.stderr)


def _is_number(value) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def deep_equal(expected, actual) -> bool:
    """JSON 值深度比较：数字按数值比（1 与 1.0 相等），bool / 数字不混。

    直接 a == b 会让 Python 把 True == 1 当成相等，这里显式排除。
    """
    if isinstance(expected, bool) or isinstance(actual, bool):
        return type(expected) is type(actual) and expected == actual
    if _is_number(expected) and _is_number(actual):
        return expected == actual
    if isinstance(expected, str) or isinstance(actual, str):
        return type(expected) is type(actual) and expected == actual
    if expected is None or actual is None:
        return expected is actual
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            return False
        return all(deep_equal(a, b) for a, b in zip(expected, actual))
    if isinstance(expected, dict) and isinstance(actual, dict):
        if expected.keys() != actual.keys():
            return False
        return all(deep_equal(expected[key], actual[key]) for key in expected)
    return type(expected) is type(actual) and expected == actual


def first_difference(expected, actual, path: str = "根") -> Optional[str]:
    """返回第一处差异的人读描述；完全相等时返回 None。"""
    if deep_equal(expected, actual):
        return None
    if isinstance(expected, dict) and isinstance(actual, dict):
        for key in sorted(set(expected) | set(actual)):
            child = "{}[{}]".format(path, json.dumps(key, ensure_ascii=False))
            if key not in expected:
                return "实际多出字段：{}".format(child)
            if key not in actual:
                return "实际缺少字段：{}".format(child)
            diff = first_difference(expected[key], actual[key], child)
            if diff is not None:
                return diff
        return None
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            return "{} 长度不同：期望 {}，实际 {}".format(path, len(expected), len(actual))
        for index in range(len(expected)):
            diff = first_difference(
                expected[index], actual[index], "{}[{}]".format(path, index)
            )
            if diff is not None:
                return diff
        return None
    return "{} 不同：期望 {}，实际 {}".format(
        path,
        json.dumps(expected, ensure_ascii=False),
        json.dumps(actual, ensure_ascii=False),
    )


def compare_with_expected(expected, stdout_text: Optional[str]) -> bool:
    """把程序 stdout 解析成 JSON 与答案文件内容深度比较；打印摘要，返回是否一致。"""
    if stdout_text is None or not stdout_text.strip():
        print(
            "比对失败：stdout 为空，无法解析 JSON"
            "（比对模式要求程序把结果写到 stdout，日志写 stderr）。",
            file=sys.stderr,
        )
        return False
    try:
        actual = json.loads(stdout_text)
    except ValueError as exc:
        print("比对失败：stdout 不是合法 JSON：{}".format(exc), file=sys.stderr)
        print("stdout 前 {} 行：".format(SUMMARY_LINES), file=sys.stderr)
        _print_truncated(stdout_text, SUMMARY_LINES)
        return False
    if deep_equal(expected, actual):
        print("比对通过：stdout 与答案文件一致。", file=sys.stderr)
        return True

    print("比对失败：stdout 与答案文件不一致。", file=sys.stderr)
    difference = first_difference(expected, actual)
    if difference is not None:
        print("第一处差异：{}".format(difference), file=sys.stderr)
    print("---- 期望（答案文件）----", file=sys.stderr)
    _print_truncated(_dumps(expected), SUMMARY_LINES)
    print("---- 实际（stdout）----", file=sys.stderr)
    _print_truncated(_dumps(actual), SUMMARY_LINES)
    return False


def _popen(command: List[str], env, capture_stdout: bool):
    """启动子进程；Windows / POSIX 都建独立进程组，超时才好整棵杀。"""
    kwargs = {"cwd": str(REPO_ROOT), "env": env}
    if capture_stdout:
        kwargs["stdout"] = subprocess.PIPE
    if os.name == "nt":
        # subprocess.CREATE_NEW_PROCESS_GROUP 在非 Windows 上不存在，用字面量。
        kwargs["creationflags"] = 0x00000200
    else:
        kwargs["start_new_session"] = True
    return subprocess.Popen(command, **kwargs)


def _kill_tree(process) -> None:
    """超时后尽力杀掉整棵进程树（cargo + 实际计算程序）。"""
    try:
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(process.pid)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        else:
            os.killpg(os.getpgid(process.pid), signal.SIGKILL)
    except (OSError, subprocess.SubprocessError):
        pass
    finally:
        try:
            process.kill()
        except OSError:
            pass


def run_program(
    name: str,
    args: List[str],
    capture_stdout: bool,
    timeout_seconds: Optional[float],
):
    """跑 cargo run（用例的 args 拼在 -- 之后）。返回 (退出码, 捕获的 stdout, 是否超时)。

    capture_stdout=False 时 stdout 直接继承（透传）；stderr 始终继承。
    """
    if not (REPO_ROOT / "Cargo.toml").is_file():
        print("错误：找不到仓库根的 Cargo.toml：{}".format(REPO_ROOT), file=sys.stderr)
        return 2, None, False

    command = [
        "cargo",
        "run",
        "--offline",
        "--release",
        "-p",
        PACKAGE,
        "--bin",
        name,
    ]
    if args:
        command.append("--")
        command.extend(args)

    env = dict(os.environ)
    # 构建纪律：不做增量（与 CI 一致）；调用方显式设置时以调用方为准。
    env.setdefault("CARGO_INCREMENTAL", "0")
    try:
        process = _popen(command, env, capture_stdout)
    except FileNotFoundError:
        print("错误：找不到 cargo，请先安装 Rust 工具链并确认它在 PATH 里。", file=sys.stderr)
        return 127, None, False
    except OSError as exc:
        print("错误：启动 cargo 失败：{}".format(exc), file=sys.stderr)
        return 2, None, False

    try:
        captured, _ = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        _kill_tree(process)
        try:
            process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        return EXIT_TIMEOUT, None, True
    except KeyboardInterrupt:
        _kill_tree(process)
        return 130, None, False

    stdout_text = None
    if captured is not None:
        stdout_text = captured.decode("utf-8", errors="replace")
    return process.returncode, stdout_text, False


def _format_cmd(cmd: List[str]) -> str:
    if not cmd:
        return "（无参数）"
    return " ".join(cmd)


def _case_mode_note(case: ComputeCase) -> str:
    if case.expected_file_missing:
        return "答案文件缺失：按无正确答案处理（只运行，输出结果，警告）"
    if case.has_expected:
        return "有正确答案文件：解析 stdout JSON 并与 {} 深度比较".format(
            case.expected_file
        )
    return "无正确答案：只运行并输出结果（视作成功）"


def _case_timeout(case: ComputeCase, global_timeout: Optional[float]) -> Optional[float]:
    if case.timeout_minutes is not None:
        return case.timeout_minutes
    return global_timeout


def execute(name: str) -> int:
    """按用例顺序执行一个计算的所有用例（串行），打印汇总，返回退出码。"""
    config, error = load_config(name)
    if error is not None:
        print("错误：计算 {!r} 的配置有问题：{}".format(name, error), file=sys.stderr)
        return EXIT_USAGE

    if config is None:
        cases = [
            ComputeCase(
                name=name + "（默认）",
                cmd=[],
                has_expected=False,
                expected=None,
                expected_file=None,
                expected_file_path=None,
                expected_file_missing=False,
                warning=None,
                timeout_minutes=None,
            )
        ]
        global_timeout = None
        warnings = [
            "没有 {}：按 1 条默认用例运行（无参数、无正确答案、视作成功）".format(
                CONFIG_FILENAME
            )
        ]
    else:
        cases = config.cases
        global_timeout = config.timeout_minutes
        warnings = list(config.warnings)

    for warning in warnings:
        print("警告：{}".format(warning), file=sys.stderr)

    results = []
    all_started = time.monotonic()
    for index, case in enumerate(cases):
        case_timeout = _case_timeout(case, global_timeout)
        print("", file=sys.stderr)
        print(
            "----- [{}] 第 {}/{} 条用例：{} -----".format(
                name, index + 1, len(cases), case.name
            ),
            file=sys.stderr,
        )
        print("命令: {}".format(_format_cmd(case.cmd)), file=sys.stderr)
        print("模式: {}".format(_case_mode_note(case)), file=sys.stderr)
        if case_timeout is not None:
            print("超时: {:g} 分钟".format(case_timeout), file=sys.stderr)

        timeout_seconds = case_timeout * 60.0 if case_timeout is not None else None
        started = time.monotonic()
        if case.has_expected:
            code, stdout_text, timed_out = run_program(
                name, case.cmd, True, timeout_seconds
            )
            # stdout 仍然透传给人看（同时用于比对）。
            if stdout_text:
                sys.stdout.write(stdout_text)
                sys.stdout.flush()
            if timed_out:
                print(
                    "错误：超过超时 {:g} 分钟，已终止。".format(case_timeout),
                    file=sys.stderr,
                )
                code = EXIT_TIMEOUT
            elif code == EXIT_OK and not compare_with_expected(case.expected, stdout_text):
                code = EXIT_MISMATCH
        else:
            code, _, timed_out = run_program(name, case.cmd, False, timeout_seconds)
            if timed_out:
                print(
                    "错误：超过超时 {:g} 分钟，已终止。".format(case_timeout),
                    file=sys.stderr,
                )
                code = EXIT_TIMEOUT

        elapsed = time.monotonic() - started
        results.append((case, code, elapsed))
        if code == EXIT_OK:
            print("结果: 通过（耗时 {:.1f}s）".format(elapsed), file=sys.stderr)
        else:
            print(
                "结果: 失败，退出码 {}（耗时 {:.1f}s）".format(code, elapsed),
                file=sys.stderr,
            )

    failed = [item for item in results if item[1] != EXIT_OK]
    warned = [item for item in results if item[0].expected_file_missing]
    total = time.monotonic() - all_started

    print("", file=sys.stderr)
    print("===== [{}] 用例汇总 =====".format(name), file=sys.stderr)
    for case, code, elapsed in results:
        state = "绿" if code == EXIT_OK else "红"
        result = "通过" if code == EXIT_OK else "失败(退出码 {})".format(code)
        if case.expected_file_missing:
            answer = "无(警告:答案文件缺失)"
        elif case.has_expected:
            answer = "有"
        else:
            answer = "无"
        print(
            "  [{}] {} | 命令: {} | 答案: {} | {} | {:.1f}s".format(
                state, case.name, _format_cmd(case.cmd), answer, result, elapsed
            ),
            file=sys.stderr,
        )
    print(
        "  合计 {} 条：绿 {}，红 {}，警告 {}，总耗时 {:.1f}s".format(
            len(results),
            len(results) - len(failed),
            len(failed),
            len(warned),
            total,
        ),
        file=sys.stderr,
    )

    if not failed:
        return EXIT_OK
    # 返回第一条失败用例的退出码（3 = 答案不一致，124 = 超时，其它 = 运行失败）。
    return failed[0][1]


def run_all() -> int:
    """依次运行所有带 compute.json 的计算（不同计算之间串行；CI 里由 matrix 并行）。"""
    names = discover_configs()
    if not names:
        print(
            "错误：{} 下没有任何带 {} 的计算。".format(
                BIN_DIR_REL.as_posix(), CONFIG_FILENAME
            ),
            file=sys.stderr,
        )
        return EXIT_USAGE

    results = []
    for name in names:
        print("", file=sys.stderr)
        print("########## {} ##########".format(name), file=sys.stderr)
        results.append((name, execute(name)))

    failed = [item for item in results if item[1] != EXIT_OK]
    print("", file=sys.stderr)
    print("===== 全部计算汇总 =====", file=sys.stderr)
    for name, code in results:
        if code == EXIT_OK:
            print("  [绿] {}".format(name), file=sys.stderr)
        else:
            print("  [红] {}（退出码 {}）".format(name, code), file=sys.stderr)
    print(
        "共 {} 个计算：绿 {}，红 {}。".format(
            len(results), len(results) - len(failed), len(failed)
        ),
        file=sys.stderr,
    )
    return EXIT_OK if not failed else EXIT_USAGE


def main(argv: List[str]) -> int:
    configure_utf8()

    if not argv:
        print("错误：缺少计算名。", file=sys.stderr)
        print(USAGE, file=sys.stderr)
        print_available()
        return EXIT_USAGE

    first = argv[0]
    if first in ("-h", "--help"):
        print(USAGE, end="")
        return EXIT_OK
    if first in ("-l", "--list"):
        if len(argv) > 1:
            print("错误：--list 后面不接受其它参数。", file=sys.stderr)
            return EXIT_USAGE
        return print_list()
    if first == "--configs":
        if len(argv) > 1:
            print("错误：--configs 后面不接受其它参数。", file=sys.stderr)
            return EXIT_USAGE
        return print_configs()
    if first == "--all":
        if len(argv) > 1:
            print("错误：--all 后面不接受其它参数。", file=sys.stderr)
            return EXIT_USAGE
        return run_all()
    if first.startswith("-"):
        print("错误：未知选项 {!r}。".format(first), file=sys.stderr)
        print(USAGE, file=sys.stderr)
        return EXIT_USAGE
    if len(argv) > 1:
        print(
            "错误：本脚本只接受一个计算名；计算参数请写进该计算目录的 {} 的 cases。".format(
                CONFIG_FILENAME
            ),
            file=sys.stderr,
        )
        print(USAGE, file=sys.stderr)
        return EXIT_USAGE

    name = first
    problem = validate_name(name)
    if problem is not None:
        print("错误：计算名 {!r} 不合法：{}。".format(name, problem), file=sys.stderr)
        print_available()
        return EXIT_USAGE

    if not main_rs_path(name).is_file():
        expected_rel = (BIN_DIR_REL / name / "main.rs").as_posix()
        print(
            "错误：找不到计算 {!r}（期望目录 {}）。".format(name, expected_rel),
            file=sys.stderr,
        )
        print_available()
        return EXIT_USAGE

    return execute(name)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
