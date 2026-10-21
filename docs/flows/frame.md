# Flow · 一帧（input → commit → MountBatch → platform）

> 改渲染、改布局、改「画面不对/跟不上」时先读这张，不要直接翻 `lib.rs`（1700+ 行）。
> 基线 `5c7060a1`（2026-10-20）· 最后核对 2026-10-21 · 置信度：确认 26 / 推断 2 / 缺口 3

---

## 1. 触发点（全部枚举）

| # | 触发方式 | 入口符号 | 文件 | 备注 |
|---|---|---|---|---|
| 1 | 平台输入消息 / 定时器到期 | `Driver::turn` [图+读码] | `crates/nsh-engine/src/lib.rs` | 引擎线程主循环；`run` 与 `LocalEngine::pump` 都落到它 |
| 2 | Web 下宿主事件循环驱动 | `LocalEngine::pump` [读码] | `crates/nsh-engine/src/lib.rs` | wasm 无独立线程，同上走 `turn` |
| 3 | `requestAnimationFrame` 那一次的帧回调 | `EngineControl::frame` → `Message::Frame` [读码] | `crates/nsh-engine/src/lib.rs` | 只跑 rAF 回调，**不直接 commit** |
| 4 | 平台自己需要一次提交（iOS 转场 / 装饰静默更新 / macOS 静默更新） | `Core::commit` 被平台直接调 [图] | `nsh-platform-ios/src/interaction.rs`、`nsh-platform-macos/src/surface.rs` | 🔴 **旁路**：不经 engine。见 §6 |
| 5 | 引擎启动（首帧） | `Engine::start_with` → `Driver::turn` [图] | `crates/nsh-engine/src/lib.rs` | `setup` 送的主题/环境在第一帧前生效 |

> 🔴 **`State::commit` 只有 1 个真实调用者**（`turn`）[图]；但 **`Core::commit` 有 148 个 CALLS 入边**，其中绝大多数是 **core 自己的测试** [图]，生产代码里的真实旁路只有表中第 4 条那几处。**别把测试的 148 当成"很多入口"。**

## 2. 阶段链

| 阶段 | 关键符号 | 文件 | 输入 | 输出 |
|---|---|---|---|---|
| ① 排队消息合并 | （`rx.try_recv` 循环）[读码] | `nsh-engine/src/lib.rs` | 已排队的所有消息 | 每帧只 `turn` 一次 |
| ② 事件派发 | `dispatch_touch_move` / `dispatch_scroll_events` / `run_component_tasks` [读码] | 同上 | 触摸、悬停、滚动、组件任务 | JS 侧状态变化 |
| ③ 提交（可多轮） | `State::commit` [图+读码] | 同上 | JS 与 core 的待办 | 一条 `MountBatch` |
| ③a 应用操作 | `Core::apply` [图] | `nsh-core/src/core/mod.rs` | `Op` 队列 | 脏节点标记 |
| ③b 一轮核心提交 | `Core::commit_round` [读码] | 同上 | 脏标记 | mount items |
| ③b-1 组件渲染 | `render_components` / `sync_back` / `sync_keys` [读码] | 同上 | 组件状态 | 视图补丁 |
| ③b-2 布局 | `flush_dirty` → `layout_main` → `layout_roots` / `layout_cells` [读码] | 同上 | 脏布局 | `Layout` 结果 |
| ③b-3 帧矩形 | `emit_frames` [读码] | 同上 | 布局结果 | `MountItem::Frame` |
| ③b-4 列表 | `update_lists` → `bind_virtual_lists` [读码] | 同上 + `components.rs` | 滚动位置 | 新的单元格 |
| ③b-5 文本 | `flush_text` [读码] | 同上 | 文本脏标记 | 文本 item |
| ④ 交给平台 | `MountSink::submit` [读码] | 各 `nsh-platform-*` | `MountBatch` | 应用进原生视图 |
| ⑤ 平台应用 | macOS：`Sink::send` → `DispatchQueue::main` → `surface.drain` [图] | `nsh-platform-macos/src/surface.rs` | 批量 | 原生视图树更新 |

> 阶段标记说明：`[图]` = 来自 `codebase-memory-mcp`；`[读码]` = 来自直接读源码。**图的 outbound 会漏掉方法调用**（见 §7），所以阶段 ③b-* 的顺序是读码得来的。

## 3. 数据契约

跨阶段传的都是 `MountBatch { items: Vec<MountItem> }`（`nsh-core/src/mount.rs`）。

| 契约点 | 必须成立 | 谁在依赖 |
|---|---|---|
| `items` **顺序即应用顺序** | 同一次提交内按序应用 | 所有平台 |
| 兄弟用 `before: Option<NodeId>` 而不是索引 | 平台自有子视图（刷新控件、滚动指示器）插入后不能错位 | 平台侧 |
| `Create` 建出来的视图**先是 detached 且不可见** | 后续 `Insert` 才挂上 | 平台侧 |
| `Overflow` 只在变化时发 | 平台靠它扩大触摸命中区（下拉框在父视图外） | UIKit / Android 点击 |
| `TrackStates` 的位与平台侧 `states & tracked` 一致 | 两边永不对某个已停止上报的状态产生分歧 | core 与平台 |
| 空提交返回 `None` | 平台不该收到空批量 | `State::commit` |

## 4. 分支矩阵

| 维度 | 取值 | 走哪条路 | 备注 |
|---|---|---|---|
| 平台 | macOS / Windows / Linux / GPUI / iOS / Android / Web | 各自实现 `MountSink` | Web 走 `LocalEngine`，无线程 |
| 有无 JS | `script: Option<&Script>` | 没有 JS 时列表不绑定、事件丢弃 | Rust 组件自己渲染 |
| 是否需要重布局 | `needs_layout` | false 时不走 `layout_main`（列表单元内的改动不动主树） | 性能关键 |
| 是否多轮 | `MAX_LIST_ROUNDS = 8`（engine）、`MAX_VIRTUAL_ROUNDS = 8`（core） | 有 list 绑定/posted/events 就再来一轮，否则 break | 两处各 8，别混 |
| 是否首帧 | `setup` 是否给了主题/环境 | 首帧前应用 | `Engine::start_with` |

## 5. 不变量

| # | 阶段边界 | 不变量 | 谁在依赖 |
|---|---|---|---|
| 1 | 进 ③ | 一次 `turn` 只产生**最多一条** `MountBatch` | 平台按一次提交处理一次刷新 |
| 2 | ③ 内各轮 | 多轮的结果**合并**进同一条批量，不是发多条 | `State::commit` 的 `merged` |
| 3 | ③b-2 后 | `needs_layout` 已清；同一帧内不会重复全树布局 | 性能基线 |
| 4 | ④ | 批量非空（空则 `None`，平台收不到） | 平台 |
| 5 | ⑤ | 平台**在 UI 线程**应用 items | macOS 用 main queue，其他同理 |
| 6 | 全程 | 节点 id 在一条批量内保持有效；`Delete` 之后不再引用 | 平台视图表 |
| 7 | ③b-5 | 文本测量结果按内容缓存；**缓存键必须覆盖影响度量的每个输入** | 见 §6 |

## 6. 缓存与记忆化点

| 缓存 | 位置 | 缓存键必须覆盖 | 失效条件 |
|---|---|---|---|
| 文本测量 | `MeasureCache`（`nsh-core/src/text.rs`） | 内容 + 样式 + 可用宽度 + 字体/字号/语言 | 字体变化（`fonts_changed`） |
| 文本整形（web） | `nsh-platform-web` 的 shape cache | 同上 + 设备像素比 | ⚠️ 曾泄漏，见 `docs/dev/text.md` |
| 布局 | Taffy 树（`layout_main`） | 脏标记驱动的增量 | `flush_dirty` |
| 列表单元格 | `update_lists` 的复用表 | 行 key + 行高 + 视口 | 滚动越界回收 |
| 类样式 | `nsh-core` 的 state 位（`TrackStates`） | 状态位必须与平台 `states & tracked` 对齐 | 停止上报时两边同时忘 |

> 🔴 **缓存键少一个输入 = 不报错、界面正常、测试不红。** 这条在 nsh PR #118 上被审了三轮才抓出来。

## 7. 未验证清单

| # | 问题 | 现状 | 怎么验证 |
|---|---|---|---|
| 1 | 平台旁路调 `Core::commit` 的三处（iOS `transition`、iOS/macOS `quiet`）是否会与 engine 的 `turn` 交错 | **未查证**。`quiet` 读码看是 `CATransaction` 包一层，`Core::commit` 的调用可能来自宏展开而非真的提交 | 在 `Core::commit` 加计数探针，跑 gallery 的转场 |
| 2 | 「一帧最多一次布局」在多轮 list 收敛下是否仍成立 | 推断成立（`needs_layout` 每轮重置），但没测 | `cargo test -p nsh-core bench_row_relayout -- --ignored` 看轮数 |
| 3 | Web 下 rAF 与 Message::Frame 的时序是否与 native 等价 | **未查证** | 对比同一 app 在 web 与 macOS 的 commit 次数 |

## 8. 可观测探针

| 想知道什么 | 在哪打点 | 怎么读 |
|---|---|---|
| 每帧耗时 / 轮数 | `State::commit` 的 `nsh::perf` 日志（`commit: N ops, Tµs, M mount items, K nodes`） | `nsh trace` 或 `Engine::set_perf_monitor` |
| 列表收敛轮数 | `MAX_LIST_ROUNDS` 循环退出条件 | 加计数 |
| 布局是否被跳过 | `layout_main` 的返回值 | `relayout` 分支 |
| 批量大小分布 | `MountBatch.items.len()` | 平台 `submit` 入口 |

---

## 维护

改这条流程的 PR **必须同时改这张图**。改完跑：

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/frame.md .
```

命中率 < 90% = 这张图不可信，先修再用。

**不要往这张图里加的东西**：调用链细节（图能算）、行号（会烂）、每个 `MountItem` 变体的完整字段（看 `mount.rs`）。

---

## 🔴 图的覆盖边界（2026-10-21 实测，用数据说话）

这张图里标 `[读码]` 的部分**不是偷懒，是图的已知盲区**。以下是 nsh 索引（v0.12.1，29k 节点 / 166k 边）的实测覆盖率：

| 文件 | 有 CALLS 出边的函数 / 总数 |
|---|---|
| `nsh-core/src/core/mod.rs`（帧的核心） | **18 / 115（15.7%）** |
| `nsh-core/src/core/components.rs` | 10 / 137（7.3%） |
| `nsh-engine/src/lib.rs` | 多数方法 0 |
| `nsh-platform-macos/src/mount.rs`（`Mounter`） | 高（`apply` 有 58 个出边） |
| `nsh-core/src/ui/js.rs` | **0 / 49** |

**结论：图在 `nsh-platform-*` 和 `nsh-ui-mobile` 上可用，在 `nsh-core` / `nsh-engine` 上几乎只够找定义。**

三条实测证据：
1. `Driver::turn`、`Driver::handle`、`Core::commit_round` 的 `outbound` 全为 **0**，但它们明明调用了别的函数。
2. 反向验证：`Surface::drain` 体内有 4 处 `self.*()` 调用，图只记了 **1** 处（`handle`），漏掉 `ensure_ticking` / `revisit` / `write_animations`。
3. `Core::commit` 的 148 个 CALLS 入边里，**145 个是 core 自己的测试**，生产代码只有 3 处（`platform-ios` 的 `interaction.transition`、`decor.quiet`，`platform-macos` 的 `surface.quiet`）。

**所以这张图的阶段链（§2 的 ③b-*）必须读码得来。** 谁若改用图来重建这段，会得到一条断掉一半的链，而且不会报错。

### 用图之前先做这一步

```bash
# 判断某个符号在图里是否可信：看它有没有出边
codebase-memory-mcp cli --quiet trace_path \
  '{"project":"Users-hxy-Documents-Github-nsh","function_name":"<限定名>","direction":"outbound","depth":1}'
# callees_total > 0  → 图可用，放心追
# callees_total == 0 → 不能判定为叶子，退回读码
```
