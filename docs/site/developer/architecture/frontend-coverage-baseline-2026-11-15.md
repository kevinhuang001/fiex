# 前端覆盖率基线

## 首次测量

2026-11-15 在 Fiex 前端完整 Vitest 套件上建立初始覆盖率记录。测量源码为 `src/**/*.{ts,tsx}`，排除测试文件和 TypeScript 声明文件；覆盖率由 Vitest V8 provider 产生。

| 指标                |                 首次测量 |
| ------------------- | -----------------------: |
| 测量源码文件        |                      301 |
| 测试文件 / 测试用例 |                103 / 507 |
| Statements          | 24.10%（6,008 / 24,920） |
| Lines               | 24.10%（6,008 / 24,920） |
| Functions           |      56.37%（407 / 722） |
| Branches            |  79.01%（2,195 / 2,778） |

源码基准是 PR #241 head `310236b4e4248b13aec66c07f24ad20b39d3fff8`。这些数字描述现有自动化测试执行到的源码比例，不代表用户功能、平台行为或故障路径的完整程度；不能据此推断未覆盖代码一定有缺陷，也不能将该比例解释为整体质量分数。

## 重复测量与趋势

- `npm run test:coverage` 运行与普通 `npm test` 相同的 Vitest 用例，并输出 `coverage/frontend/coverage-summary.json` 和 `coverage/frontend/lcov.info`。
- `npm run report:coverage` 将当前结果与 `scripts/coverage-baseline.json` 对比，生成 `coverage/frontend/coverage-trend.md`。
- CI 在前端测试 job 中上传 JSON 汇总、趋势 Markdown 和 LCOV 文件，artifact 名为 `frontend-coverage`。这项报告目前不设门槛，不改变既有测试通过条件。
- 新增或实质修改业务逻辑时，需求追溯仍应指出对应的行为测试。达到稳定的差异覆盖率样本后，再为适合的模块讨论门槛。

本次变更只建立前端覆盖率基线。Rust 全 workspace 覆盖率仍未测量；Scope Policy 的现有 85% 行覆盖门槛继续独立执行。
