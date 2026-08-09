#!/usr/bin/env python3
"""差分测试框架（A3）：对比 Rust 版输出与 Python akshare 输出。

用法：
  # 生成 golden fixture（需要 Python akshare 环境 + 网络）
  python3 tools/parity_runner.py --generate
  python3 tools/parity_runner.py --generate --only stock_zh_a_hist

  # 运行对比（需要 Rust parity bin + 网络）
  python3 tools/parity_runner.py --check
  python3 tools/parity_runner.py --check --only stock_zh_a_hist

  # 仅查看用例清单
  python3 tools/parity_runner.py --list

设计说明：
- 用例注册表 CASES：每个用例 = 函数名 + 参数 + 对比模式
- 对比模式 strict：列名/dtype/行数/head 值全部一致
- 对比模式 loose：仅列名与列数一致（实时行情类数据，值随时间变化）
- golden fixture 保存在 tests/golden/{func}.json（列名/dtype/行数/head）
- 对比容忍 float 字符串化差异（pandas 与 Rust 浮点打印规则不同）

退出码：0 = 全部通过或跳过；1 = 存在对比失败。
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import warnings

warnings.filterwarnings("ignore")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN_DIR = os.path.join(ROOT, "tests", "golden")
PARITY_BIN = os.path.join(ROOT, "target", "debug", "parity")
HEAD_N = 5

# 用例注册表：函数名 → (参数, 对比模式, 说明)
# 参数与 Rust parity bin / Python akshare 同名函数的参数一致（全字符串）。
CASES: list[tuple[str, list[str], str, str]] = [
    ("stock_zh_a_hist", ["000001", "daily", "20240101", "20240131", ""], "strict", "A股日K历史"),
    ("stock_zh_a_hist_min_em", ["000001", "2026-01-01 09:00:00", "2026-12-31 15:00:00", "5", ""], "strict", "A股5分钟K线"),
    ("stock_individual_info_em", ["000001"], "strict", "个股信息"),
    ("stock_bid_ask_em", ["000001"], "strict", "五档盘口"),
    ("stock_board_industry_name_em", [], "loose", "行业板块列表"),
    ("stock_board_concept_name_em", [], "loose", "概念板块列表"),
    ("stock_board_industry_cons_em", ["小金属"], "loose", "行业板块成分"),
    ("stock_board_concept_cons_em", ["昨日连板"], "loose", "概念板块成分"),
    ("stock_board_industry_hist_em", ["小金属", "20240101", "20240131", "日K"], "strict", "行业板块历史"),
    ("stock_board_concept_hist_em", ["昨日连板", "daily", "20240101", "20240131", ""], "strict", "概念板块历史"),
    ("stock_zt_pool_em", ["20240105"], "strict", "涨停股池"),
    ("stock_individual_fund_flow", ["000001", "sh"], "strict", "个股资金流"),
    ("stock_lhb_detail_em", ["20240101", "20240131"], "strict", "龙虎榜详情"),
    ("stock_hsgt_fund_flow_summary_em", [], "loose", "沪深港通资金流"),
    ("stock_gpzy_profile_em", [], "loose", "股权质押统计"),
    ("stock_zh_a_spot_em", [], "loose", "A股实时行情"),
    ("stock_sh_a_spot_em", [], "loose", "沪A实时行情"),
    ("stock_sz_a_spot_em", [], "loose", "深A实时行情"),
    ("index_zh_a_hist", ["000001", "daily", "20240101", "20240131"], "strict", "指数日K"),
    ("index_zh_a_hist_min_em", ["399006", "5", "2026-01-01 09:00:00", "2026-12-31 15:00:00"], "strict", "指数分钟K线"),
    ("fund_etf_spot_em", [], "loose", "ETF实时行情"),
    ("fund_lof_spot_em", [], "loose", "LOF实时行情"),
    ("fund_etf_hist_em", ["510300", "daily", "20240101", "20240131", ""], "strict", "ETF日K"),
    ("stock_profile_cninfo", ["600030"], "strict", "巨潮公司概况"),
    ("stock_ipo_summary_cninfo", ["600030"], "strict", "巨潮上市相关"),
    ("stock_dividend_cninfo", ["600009"], "strict", "巨潮历史分红"),
    ("stock_new_ipo_cninfo", [], "strict", "巨潮新股发行"),
    # stock_new_gh_cninfo: akshare 在空数据时 pd.DataFrame([]) 设置列名报
    # Length mismatch（上游 bug），无法生成 golden；Rust 侧已离线验证空表列契约
]


def pandas_dtype(dtype) -> str:
    """pandas dtype → 简化五类（与 Rust export_parity 对齐）。"""
    name = str(dtype)
    if name.startswith("int"):
        return "int64"
    if name.startswith("float"):
        return "float64"
    if name.startswith("bool"):
        return "bool"
    if name.startswith("datetime"):
        return "datetime"
    return "str"


def py_contract(func: str, args: list[str]) -> dict:
    """调用 Python akshare 同名函数，输出与 Rust export_parity 同构的契约。"""
    import akshare as ak

    fn = getattr(ak, func)
    df = fn(*args)
    columns = [
        {"name": str(c), "dtype": pandas_dtype(df[c].dtype)} for c in df.columns
    ]
    head: list[list] = []
    for _, row in df.head(HEAD_N).iterrows():
        cells = []
        for c in df.columns:
            v = row[c]
            if v is None or (isinstance(v, float) and v != v):  # NaN
                cells.append(None)
            else:
                cells.append(str(v))
        head.append(cells)
    return {"ok": True, "columns": columns, "height": int(len(df)), "head": head}


def rust_contract(func: str, args: list[str]) -> dict:
    """调用 Rust parity bin，解析契约 JSON。"""
    proc = subprocess.run(
        [
            PARITY_BIN,
            "--func",
            func,
            "--args",
            json.dumps(args),
            "--head",
            str(HEAD_N),
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if proc.returncode != 0:
        return {"ok": False, "error": f"parity bin 退出码 {proc.returncode}: {proc.stderr[:300]}"}
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        return {"ok": False, "error": f"parity bin 输出非 JSON: {e}; stdout={proc.stdout[:200]}"}


def load_golden(func: str) -> dict | None:
    path = os.path.join(GOLDEN_DIR, f"{func}.json")
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    return None


def save_golden(func: str, contract: dict) -> None:
    os.makedirs(GOLDEN_DIR, exist_ok=True)
    with open(os.path.join(GOLDEN_DIR, f"{func}.json"), "w", encoding="utf-8") as f:
        json.dump(contract, f, ensure_ascii=False, indent=2)


def norm_val(v) -> str | None:
    """归一化单元格值用于比较（去除浮点尾差）。"""
    if v is None:
        return None
    s = str(v).strip()
    if s in ("nan", "None", "NaT", ""):
        return None
    try:
        f = float(s)
        # 统一浮点表示：整数不带小数点；其余保留 6 位有效精度
        if f == int(f) and abs(f) < 1e15:
            return str(int(f))
        return f"{f:.6f}".rstrip("0").rstrip(".")
    except ValueError:
        return s


def compare(func: str, golden: dict, actual: dict, mode: str) -> list[str]:
    """对比两个契约，返回失败项列表。"""
    issues: list[str] = []
    if golden.get("ok") is not True:
        return [f"golden 生成失败: {golden.get('error')}"]
    if actual.get("ok") is not True:
        return [f"rust 执行失败: {actual.get('error')}"]

    g_cols, a_cols = golden["columns"], actual["columns"]
    if [c["name"] for c in g_cols] != [c["name"] for c in a_cols]:
        issues.append(
            f"列名不一致\n  python: {[c['name'] for c in g_cols]}\n  rust:   {[c['name'] for c in a_cols]}"
        )
    # dtype 归一化：pandas 自动推断的 int64/float64 视为同一数值类（值仍严格比较）；
    # pandas 的 datetime64 与我们的 ISO 日期字符串表示等价（值仍严格比较）
    def norm_dtype(d):
        if d in ("int64", "float64"):
            return "num"
        if d in ("datetime", "str"):
            return "str"
        return d

    g_dt = [norm_dtype(c["dtype"]) for c in g_cols]
    a_dt = [norm_dtype(c["dtype"]) for c in a_cols]
    if g_dt != a_dt:
        issues.append(
            f"dtype 不一致\n  python: {[c['dtype'] for c in g_cols]}\n  rust:   {[c['dtype'] for c in a_cols]}"
        )

    if mode == "strict":
        if golden["height"] != actual["height"]:
            issues.append(f"行数不一致: python={golden['height']} rust={actual['height']}")
        g_head, a_head = golden["head"], actual["head"]
        for i, (grow, arow) in enumerate(zip(g_head, a_head)):
            g_norm = [norm_val(v) for v in grow]
            a_norm = [norm_val(v) for v in arow]
            if g_norm != a_norm:
                issues.append(f"head 第 {i} 行不一致\n  python: {g_norm}\n  rust:   {a_norm}")
                break
    return issues


def main() -> int:
    ap = argparse.ArgumentParser(description="parity 差分测试")
    ap.add_argument("--generate", action="store_true", help="生成 golden fixture")
    ap.add_argument("--check", action="store_true", help="对比 golden 与 rust 输出")
    ap.add_argument("--only", help="仅运行指定函数")
    ap.add_argument("--list", action="store_true", help="列出用例")
    args = ap.parse_args()

    if args.list:
        for func, params, mode, desc in CASES:
            print(f"{func}({', '.join(params) or '-'})  [{mode}]  {desc}")
        return 0

    cases = CASES
    if args.only:
        cases = [c for c in CASES if c[0] == args.only]
        if not cases:
            print(f"未知函数: {args.only}")
            return 2

    failures = 0
    skipped = 0
    for func, params, mode, desc in cases:
        label = f"{func}({', '.join(params) or '-'}) [{mode}] {desc}"

        if args.generate:
            try:
                contract = py_contract(func, params)
                save_golden(func, contract)
                status = "生成" if contract.get("ok") else "失败"
                detail = (
                    f"{len(contract['columns'])} 列 x {contract['height']} 行"
                    if contract.get("ok")
                    else contract.get("error")
                )
            except Exception as e:  # noqa: BLE001
                contract = {"ok": False, "error": str(e)[:200]}
                status = "异常"
                detail = str(e)[:200]
            if not contract.get("ok"):
                failures += 1
            print(f"[{status}] {label} -> {detail}")

        if args.check:
            golden = load_golden(func)
            if golden is None:
                print(f"[跳过] {label} (无 golden fixture，先运行 --generate)")
                skipped += 1
                continue
            actual = rust_contract(func, params)
            issues = compare(func, golden, actual, mode)
            if issues:
                failures += 1
                print(f"[失败] {label}")
                for it in issues:
                    print(f"       {it}")
            else:
                print(
                    f"[通过] {label} ({len(golden['columns'])} 列 x {golden['height']} 行)"
                )

    print(f"\n汇总: {'失败' if failures else '全部通过'} (失败 {failures}, 跳过 {skipped})")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
