# akshare-rust

Rust 版 [akshare](https://github.com/akfamily/akshare)：纯 HTTP + 内置 JS 引擎的财经数据获取库。

数据获取**完整参照 akshare 的技术实现方式**（v1.0 不使用浏览器）：

- 纯 HTTP 请求（`reqwest` blocking）＋ UA 伪装 ＋ 指数退避重试 ＋ 多节点容灾
- 内置 JS 引擎（`rquickjs`/QuickJS）执行网站下发的加密脚本，
  等价于 akshare 用 `py_mini_racer`（V8）执行同一份 JS（已实测输出逐字符一致）
- 数据返回为 `Df`（polars DataFrame），列名与 akshare 逐字对齐

## 快速开始

```bash
cargo build
cargo run --bin demo    # 真实网络冒烟测试
cargo test              # 离线单测（含 JS 引擎与数据管线）
```

```rust
use akshare_rust::stock::stock_zh_a_hist;

let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "qfq")?;
println!("{}", df);
```

## 已实现接口（首批）

| 函数 | 对应 akshare | 数据源 |
|---|---|---|
| `stock_zh_a_hist` | `ak.stock_zh_a_hist` | 东财 K 线 |
| `stock_zh_a_hist_min_em` | `ak.stock_zh_a_hist_min_em` | 东财分钟 K 线/分时 |
| `stock_zh_a_spot_em` / `stock_sh_a_spot_em` / `stock_sz_a_spot_em` / `stock_bj_a_spot_em` | `ak.stock_*_spot_em` | 东财行情列表 |
| `stock_individual_info_em` | `ak.stock_individual_info_em` | 东财个股信息 |
| `stock_bid_ask_em` | `ak.stock_bid_ask_em` | 东财五档盘口 |
| `index_zh_a_hist` | `ak.index_zh_a_hist` | 东财指数 K 线 |
| `index_zh_a_hist_min_em` | `ak.index_zh_a_hist_min_em` | 东财指数分钟 K 线/分时 |
| `index_code_id_map_em` | `ak.index_code_id_map_em` | 东财指数映射 |
| `fund_etf_hist_em` | `ak.fund_etf_hist_em` | 东财 ETF K 线 |
| `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_etf_spot_em` / `ak.fund_lof_spot_em` | 东财基金行情列表 |

> 完整实施计划见 [`PLAN.md`](PLAN.md)（1099 个函数 / 33 个分类的迁移路线图）。

## 架构

```
src/
├── core/           # 基础设施
│   ├── error.rs    # AkshareError 统一错误类型（Empty/Js/Blocked/AuthRequired/Status/Http...）
│   ├── config.rs   # 全局配置（UA/超时/重试/代理）
│   ├── http.rs     # reqwest 封装：指数退避+抖动重试、多节点容灾、字符集解码、反爬特征检测
│   ├── df.rs       # Df（polars DataFrame 封装）：JSON 建表/排序/列转换，列序对齐 pandas
│   └── js_engine.rs# rquickjs 封装：eval 加密 JS + 浏览器全局 shim 注入
├── sources/        # 数据源层（一个源一个模块）
│   └── eastmoney.rs# 东财：clist 分页（多节点故障转移）/ K 线 / 市场判定
├── stock/          # 股票接口（对应 akshare stock_* 函数）
├── index/          # 指数接口（对应 akshare index_* 函数）
├── fund/           # 基金接口（对应 akshare fund_* 函数）
└── bin/demo.rs     # 命令行冒烟演示
```

### 关键设计

- **多节点容灾**：东财 push2 集群单节点可能被限流/故障，`fetch_paginated_diff_any` /
  `get_json_any` 第一轮每节点单次快速探测、失败立即切换，全部失败后再按完整重试策略兜底。
- **分钟级数据滚动窗口**：东财分钟 K 线/分时接口只返回最近约 8 个月的滚动数据，
  与 akshare 行为一致；请求较早日期的分钟数据会得到空表。
- **JS 加密**：一律用 rquickjs 执行 akshare 原版 JS，不在 Rust 手写算法；
  通过注入 `var BROWSER_LIST; var time;` 等浏览器全局 shim 兼容非严格模式写法。
- **反爬识别**：响应含 `_waf`/`Just a moment`/`challenge-platform` 判为 `Blocked`，
  含 `400016`/`xq_a_token` 等判为 `AuthRequired`，明确报错而非返回脏数据。
- **4xx 不重试**：客户端错误立即返回；仅 5xx 与连接错误进入退避重试
  （对应 akshare `raise_for_status` 语义）。

## 开发规范

- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` 必须零告警
- 无 `unwrap`/`expect`（除 `Client::build` 等构造点外），错误统一走 `Result<AkshareError>`
- 公开函数必须带 `///` 文档注释（参数、返回列）
- 数据变换逻辑抽成纯函数并配离线单测（不依赖网络）

## 已知限制

- 东财 push2 集群对本机 IP 有临时限流（表现为 TLS close_notify 连接重置），
  Python akshare 同样受影响；容灾与重试会尽量规避，必要时稍后重试。
