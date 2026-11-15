# Rust workspace 覆盖率基线

## 首次测量

2026-11-15 使用 LLVM source-based coverage 和 `cargo-llvm-cov` 对 Rust workspace 的 library unit tests 建立初始基线。命令使用 `--workspace --lib --no-default-features --features fiex/usbfs`，与本地 Rust workspace 测试的 Linux/no-desktop feature 组合一致；Coverage job 固定 `PROPTEST_RNG_SEED=20261115` 并串行运行 test harness，减少随机属性输入和测试调度对样本的影响。

| 指标         |                  首次测量 |
| ------------ | ------------------------: |
| 测量源码文件 |                       376 |
| Lines        | 55.31%（22,628 / 40,911） |
| Functions    |   50.09%（2,563 / 5,117） |
| Regions      | 52.29%（33,868 / 64,771） |

基准源码为 PR #242 head `d0bd4c6f2ee3e605e2a024d46cc44b0d15342f21`。本测量不包括 doctests、binary targets、integration tests 或独立的 SDK submodule。当前稳定工具链 profile 不报告 branch 和 MC/DC 覆盖率；表中没有这两个指标，也不以零值代替“未测量”。

覆盖率只表示本次 unit test 执行到的 Rust library 源码比例。重复串行测量的 region 计数曾相差 1（不足 0.01 个百分点，按报告精度均为 52.29%）；趋势以两位小数呈现且不设门槛。它不是平台行为完整性或整体质量分数，尤其不能替代 macOS、Windows 和 Linux 的原生测试。

## 重复测量与趋势

- CI 的 `Rust workspace coverage (Linux)` job 使用锁定的 Rust toolchain 与 Cargo lockfile 运行 workspace library unit tests。
- job 发布 `coverage/rust/rust-coverage.json` 和 `coverage/rust/rust-coverage-trend.md` artifact；趋势按 Lines、Functions、Regions 与此基线比较。
- 趋势报告不设阈值，不改变现有 Scope Policy 的 85% 行覆盖率门槛。
- 当前基线是 Linux/no-desktop 的可重复口径，不声称代表其他操作系统或桌面 feature 组合。
