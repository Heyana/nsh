---
date: 2026-10-21
project: nsh-text-metrics
phase: PR 就绪待提交
status: ready
sha: 33776f6a51a6e1dd07880ffc8343527a3a980348
branch: fix/multiline-text-metrics
from: workbuddy-agent
to: next-session
machine: Mac mini
knowledge_updated: knowledge/nsh/nsh-workspace-layout.md
---

# 交接：`leading-[1.72]` 多行文本塌陷修复（PR 待提交）

## 一句话状态

分支 `fix/multiline-text-metrics` 已就绪：2 个提交、工作区干净、回归测试通过、与 main 无冲突。**唯一未做的是反向验证**（撤掉修复应让测试失败），被磁盘满挡住。明天第一件事就是它。

## 已完成

| 内容 | 提交 / 位置 |
|---|---|
| 修复：裸数字按字号倍数解析，`px/pt/rem` 仍是点数 | `5e29b1be` · `crates/nsh-core/src/tailwind/utilities.rs` (+8 行) |
| 回归测试 `tailwind::tests::leading_arbitrary` | `5e29b1be` · `crates/nsh-core/src/tailwind/tests.rs` (+16 行) |
| gallery Line height 卡片 + 测试 | `33776f6a` · `examples/gallery/demos/TextDemo.tsx`、`examples/gallery/tests/text-line-height.test.tsx` |

**Bug 本体**（longbridge/nsh#289）：`leading-[1.72]` 原走 `length_value` 被当作 **1.72pt**；macOS CoreText 把首 span 的行高当作 `MinimumLineHeight`/`MaximumLineHeight`，于是每行压到 1.7pt、整块约 6pt 高、**画不出来**。

## 关键决策

- **裸数字 = 倍数**，与 CSS 及既有命名工具类 `leading-*` 保持一致；带单位 `px/pt/rem` 仍为点数。改动只在 `PREFIXES` 表里 `leading` 分支之前加 8 行早退，**未动 `length_value` 本身**（避免影响其他 utility）。
- **两处测试分层**：`nsh-core` 单测锁解析层，gallery 集成测试锁渲染层（走到卡片确认三段布局都在）。

## 坑与经验沉淀

见 `knowledge/nsh/nsh-workspace-layout.md`（本次新建）与今日项目日志。三条要点：

1. **0% CPU 挂住 ≠ 死锁，先 `df -h`。** 本机磁盘满时 `cc`/`rustc` 写不出文件就挂住，与死锁表现完全一致。
2. **cargo 的 `has been running for over 60 seconds` 是提示不是超时。** 判"卡死"要看 `ps` 的 CPU 累计时间是否增长，再 `sample <pid>` 抓栈。
3. **别在测试跑着的时候 `pkill`。** SIGTERM 会被记成测试失败，回头会让自己怀疑代码。

## 环境快照（关键行）

```
MACHINE: Mac mini
PROJECT_ROOT: nsh-text-metrics（当前 cwd）
RUST: stable-aarch64-apple-darwin
DISK: 根分区 228Gi / 已用 198Gi / 剩余约 1.2Gi  ← 🔴 100% 满，开工前必须先清
```

## 资产清单

### 目录拓扑（5 项 = 1 主仓 + 4 条工作线）

```
Documents/Github/
├── nsh/                 🟢 主仓（main，f40dea53）
├── nsh-text-metrics/    🔵 本文档所在，分支 fix/multiline-text-metrics
├── nsh-cpu2/            🟢 线 fix/taffy-skip-unchanged
├── nsh-fix/             🟢 线 fix/text-remount-size-stable
└── nsh-pr/              🟢 线 pr/a11y-changed-attrs
```

**以上 5 个都是同一仓库的不同 worktree**（主仓相同）。🔴 **权威查法**：

```bash
git -C Documents/Github/nsh worktree list    # 应输出 5 项
```

**不要用 `ls`/`find` 数**：worktree 是兄弟目录，仓库内搜索根本看不到；且残留目录会让数字虚高。

### 资产

| 资产 | 相对路径 | 校验命令 |
|---|---|---|
| 修复 diff | `crates/nsh-core/src/tailwind/utilities.rs` | `git -C Documents/Github/nsh-text-metrics show 5e29b1be --stat` |
| 回归测试 | `crates/nsh-core/src/tailwind/tests.rs` | `cargo test -p nsh-core leading_arbitrary`（应 1 passed） |
| gallery 卡片 | `examples/gallery/demos/TextDemo.tsx` | `git show 33776f6a --stat` |
| 构建产物 `target/` | `target/`（**19G，体积随构建浮动**） | 存在性即可；重建 `cargo build` |

## Runbook 交付

🔴 **`AGENTS.md` 不存在，本项目无 `## Runbook` 段。** 接手方按下列命令跑：

```bash
# 跑测试（回归用例）
cargo test -p nsh-core leading_arbitrary

# 跑 gallery 集成测试
cargo test -p gallery text_line_height

# 全量（慢：apps_still_run 单独就要 556s，见坑 #2）
cargo test --workspace --exclude nsh-gallery --no-fail-fast
```

⚠️ **开工前先 `df -h /System/Volumes/Data`**，剩余 <5G 先清 `target/debug/incremental`。

## Search Hints

| 类型 | 值 |
|---|---|
| Key Files | `crates/nsh-core/src/tailwind/utilities.rs`（`PREFIXES` 表的 `leading` 分支）<br>`crates/nsh-core/src/tailwind/tests.rs`<br>`examples/gallery/demos/TextDemo.tsx` |
| Grep Patterns | `grep -n 'leading-\[1.72\]' -r .`<br>`grep -n 'bare number is a multiple' crates/nsh-core/src/tailwind/utilities.rs` |
| Symbols | `PREFIXES`、`arbitrary()`、`css::number()`、`length_value()`、`K::LineHeight`、`sets()` |
| 上游对比 | 修复前行为可从 `git show 5e29b1be^:crates/nsh-core/src/tailwind/utilities.rs` 取 |

## 关联任务

`<!-- td: none -->` — 本次接手时未建卡，纯会话内推进。

## 未决问题与阻塞

| # | 问题 | 倾向 |
|---|---|---|
| 1 | 🔴 **磁盘 1.2Gi，100% 满**。`target` 独吞 19G；`~/Library/Android` 8.5G；`~/Documents/Github` 其他项目约 20G | 删 `target` 重编，或至少清 `incremental`（4.1G） |
| 2 | ⚠️ **反向验证未做**：撤掉 `utilities.rs` 的 8 行修复后，`leading_arbitrary` 应当失败 —— 目前只有 PR 描述里的声明，**没有实测证据** | 清出空间后补做（worktree 隔离，别动主分支） |
| 3 | PR 落后 main 13 个提交（无冲突） | 可 rebase 后提，或直接提让 CI 跑 |

## 下一步

- [ ] `df -h /System/Volumes/Data`，确认剩余 ≥5G
- [ ] `rm -rf target/debug/incremental`（安全，纯缓存，不影响产物）
- [ ] 建 worktree 做反向验证：`git worktree add /tmp/nsh-rv 5e29b1be^ --detach` → cherry-pick 只取 `tests.rs` → `cargo test -p nsh-core leading_arbitrary` **应失败**
- [ ] 若失败成立，恢复 worktree 后 `git push origin fix/multiline-text-metrics` 并开 PR，描述引用 longbridge/nsh#289
- [ ] PR 描述里写清：裸数字=倍数、`px/pt/rem`=点数、回归测试位置

<!-- td: none -->
<!-- machine: Mac mini -->
<!-- knowledge: knowledge/nsh/nsh-workspace-layout.md -->
