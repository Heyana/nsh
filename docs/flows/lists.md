# Flow · 虚拟化列表（window → bind → measure → anchor → recycle）

> 改列表卡顿、错行、跳动、`scrollToIndex` 落不准时先读这张。
> 上游是 `docs/flows/frame.md` 的 §2 阶段 ③b-4（`update_lists`），这张只负责它之后到回收为止的那一段。
> 基线 `57b81b5e`（2026-10-21）· 最后核对 2026-10-21 · 置信度：确认 30 / 推断 3 / 缺口 6

---

## 1. 触发点（全部枚举）

| # | 触发方式 | 入口符号 | 文件 | 备注 |
|---|---|---|---|---|
| 1 | 平台报告滚动位置 | `EngineControl::scroll` → `Message::Scroll` → `Core::set_scroll_offset` → `ListState::reported` [读码] | `crates/nsh-engine/src/lib.rs` / `crates/nsh-core/src/core/mod.rs` / `crates/nsh-core/src/core/list.rs` | 只有 offset 真变了才 `dirty`，见 §5-4 |
| 2 | JS 改了行数 / 行身份 | `Core::apply` 的 `Op::SetProp`（`Prop::Count` / `Prop::Splice` / `Prop::EstimatedItemSize` / `Prop::Columns`）[读码] | `crates/nsh-core/src/core/mod.rs` | `Prop::Splice` 只带 `(at, removed, inserted)`，不带行数据 |
| 3 | 单元格内容到位 | `Op::SetProp` 的 `Prop::Slot` / `Prop::CellIndex` [读码] | 同上 | `CellIndex` 是**确认**，只有确认过的 cell 才参与测量 |
| 4 | Rust 组件的行变了 | `update_virtual_list` → `send_list_rows` / `set_list_rows` [读码] | `crates/nsh-core/src/core/components.rs` | 组件自己的 `virtual_list`，不走 JS |
| 5 | 滚动命令 | `Command::ScrollToIndex` / `Command::ScrollToEnd` / `Command::ScrollTo` [读码] | `crates/nsh-core/src/core/mod.rs` | 在 `update_lists` 里被消费并转成 `ScrollTo` + pin |
| 6 | 列表自己或祖先重布局 | `commit_round` 的 `relayout`（`self.needs_layout`）[读码] | 同上 | `relayout == true` 时每个 list 都重算 |
| 7 | 新建 list | `ListState::new` 里 `dirty: true` [读码] | `crates/nsh-core/src/core/list.rs` | 首帧必出一次窗口 |

> 🔴 **`Core::commit` 在图里只有 3 个 CALLS 入边**（iOS `interaction.transition` / iOS `decor.quiet` / macOS `surface.quiet`）[图]。engine 的 `Driver::turn` → `State::commit` → `Core::commit` 这条链**图里没有边**（`State::commit` 的 callers 只有 `run_app` 和一个文件级节点）[图]。所以本图的阶段链全部读码得来。

## 2. 阶段链

### 2.1 一轮（round）内部

`Core::commit` 的结构是一层嵌套：外层一次 `commit_round`，然后**内层最多 8 轮**只服务 Rust 组件的列表 [读码]：

| 阶段 | 关键符号 | 文件 | 输入 | 输出 |
|---|---|---|---|---|
| ① 首轮提交 | `commit_round` [读码] | `crates/nsh-core/src/core/mod.rs` | 上一条消息产生的 op | 布局 + 首批 binds |
| ② 内层收敛 | `commit` 的 `MAX_VIRTUAL_ROUNDS` 循环 → `bind_virtual_lists` → `commit_round` [读码] | 同上 + `crates/nsh-core/src/core/components.rs` | 组件列表的 binds | 组件列表的新 cell |
| ②-1 分流 | `bind_virtual_lists` 的 `retain` [读码] | `crates/nsh-core/src/core/components.rs` | `Core::list_binds` 整条队列 | 组件 bind 被消费，**JS bind 原样留在队列里** |
| ②-2 造 cell | `bind_virtual_cell` [读码] | 同上 | `(slot, index)` | `Op::Create` / `Op::Insert` / `Prop::Slot` / `Prop::CellIndex` 四个 op |
| ③ 交给 JS | `Core::take_list_binds` → `State::commit` 的 `self.binds` → `Script::bind_list` → `Host::bind_list` → `list::bind` [图+读码] | `crates/nsh-core/src/core/mod.rs` / `crates/nsh-engine/src/lib.rs` / `crates/nsh-engine/src/script.rs` / `crates/nsh-js/src/host.rs` | 剩下的 `ListBind` | 行内容 + `cellIndex` |
| ④ 造 JS cell | `make_cell` [读码] | `crates/nsh-js/src/list.rs` | 首次出现的 slot | `Tag::Cell` 节点 + 行内容 |
| ⑤ 复用 JS cell | `list::bind` 的 `reactive::set_signal` 分支 [读码] | 同上 | 已有 slot | 只改 index signal |
| ⑥ 确认 | `list::bind` 末尾批量设 `cellIndex` [读码] | 同上 | 所有 `(slot,index)` | `Prop::CellIndex` op |
| ⑦ 回灌 | `State::commit` 的 `script.take_ops` → `Core::apply` [读码] | `crates/nsh-engine/src/lib.rs` | JS 的 op | `ListState::rendered` 被写 |
| ⑧ 布局 cell | `layout_cells` [读码] | `crates/nsh-core/src/core/mod.rs` | 脏 cell（各自是 Taffy root） | cell 真尺寸 |
| ⑨ 测量 + 定窗 | `update_lists` → `ListState::measured` / `update_window` [读码] | 同上 + `crates/nsh-core/src/core/list.rs` | cell 尺寸 | `ListState::rows`（Fenwick）、新 binds、`ScrollBy` |
| ⑩ 摆放 | `update_lists` 里的 `place` 闭包 + `emit_frames` [读码] | `crates/nsh-core/src/core/mod.rs` | 测量值 | `MountItem::Frame` |

### 2.2 两处 8 轮为什么会互相驱动

| 层 | 常量 | 位置 | 一轮做什么 | 什么时候退出 |
|---|---|---|---|---|
| core 内层 | `MAX_VIRTUAL_ROUNDS` = 8 | `crates/nsh-core/src/core/mod.rs` | `bind_virtual_lists()` 造的是**组件**的 cell，然后就地 `commit_round()` 再测 | `bind_virtual_lists()` 返回 `false`（队列里没有组件 bind） |
| engine 外层 | `MAX_LIST_ROUNDS` = 8 | `crates/nsh-engine/src/lib.rs` | 把 `self.binds` 交给 JS、跑 posted、派事件、收 op、`Core::commit()`、`take_list_binds` | `binds`、`events` 都空且没有 posted |

驱动关系（一句话）：**两个循环共用同一条 `Core::list_binds` 队列，内层只拿走"自己能造"的那部分，剩下的正好是外层的输入。**

- 内层不能自己收敛 JS 的列表：JS 的 cell 内容只能由 JS 运行时渲染，而 `Core` 是同步的、没有回调进 JS 的路径 `[读码]`。所以 `bind_virtual_lists` 的判据是 `components.virtual_lists.contains_key(&bind.list)` —— **认的是"谁拥有行"，不是 `Tag::List`** `[读码]`。
- 外层不能无限重试：`MAX_LIST_ROUNDS` 就是为"永远收敛不了的列表"设的闸 `[读码]`；剩下的留在 `self.binds` 里等下一次 `State::commit`。
- 两层会互相触发：外层下一轮又调 `Core::commit`，内层重新从第 ① 步开始；内层某一轮改变了布局，JS 那边的窗口位置（依赖 `content_len` 与 offset）也跟着变，于是外层下一轮又有 bind。
- 数量级：单次 `Core::commit` = 1 + ≤8 = **≤9 个 `commit_round`**；单帧 ≤8 次 `Core::commit` → 理论上限 **72 个 `commit_round`（72 次布局）** `[读码]`。这是上界，不是常态。
- 常态：只有 JS 的应用从不进内层循环（`bind_virtual_lists` 第一次就返回 false）；一次会产生 bind 的帧**至少 2 个外层轮**（第 1 轮产 bind，第 2 轮才把它们交给 JS），所以每次重绑 = 2 次 `Core::commit` `[读码]`。

> 🔴 两处都是 8 但语义不同：`MAX_VIRTUAL_ROUNDS` 是**同一次 `Core::commit` 内**的重入上限，`MAX_LIST_ROUNDS` 是**同一帧内**的重入上限。调其中任何一个都会改变另一个的放大倍数。

### 2.3 行高测量与列表收敛的先后关系

- `commit_round` 的顺序是 `render_components` → `flush_dirty` → `layout_main` / `layout_roots` / `layout_cells` → `update_lists` → `flush_text` `[读码]`。**测量永远在本轮布局之后**：cell 的尺寸来自 `layout_cells` 跑出来的 Taffy root 尺寸（`update_lists` 里 `self.layout.layout(cell_layout)`）。
- 但 bind 是 `update_lists` 的**产物**，所以本轮产出的 bind，内容要到下一轮才存在。于是必然有：
  `measure(N) → window(N) → bind(N) → content(N+1) → layout(N+1) → measure(N+1)` `[读码]`
  即**每一波 bind 至少吃掉一轮**，这是两处 8 轮存在的全部理由。
- 未渲染过的行永远只有估计值（`ListState::estimated`，默认 50）；尺寸表是"NaN → 实测值"的 Fenwick 前缀和，`rebuild_offsets` 在行数/列数/估计值变化时重建 `[读码]`。所以窗口是按"估计模型"算出来的，每轮实测都在修这个模型。
- 只有 `Slot::ready`（`index == rendered` 且 cell 已存在）的 cell 参与测量与定位 `[读码]`：**没确认的 cell 对测量是不可见的**，既不加分也不减分。
- 两条路的确认时机不同：组件的 cell 由 `bind_virtual_cell` 在同一轮末尾自己写 `Prop::CellIndex`，`rendered` 立刻等于 `index`；JS 的 cell 要等 JS 把 `cellIndex` 当 op 送回来，所以**JS 的 cell 比组件的 cell 晚一轮进入测量** `[读码]`。
- 文本高度在布局期就定了（`layout_root` 的 measure 闭包），`flush_text` 只是在本轮末尾发 `MountItem::Text`，不改变 cell 尺寸 `[读码]`。所以"文本比行高晚到"不是问题，"文本比测量晚到"才是。

## 3. 数据契约

| 契约点 | 必须成立 | 谁在依赖 |
|---|---|---|
| `ListBind { list, binds }` 的 `binds` 是 `(slot, index)` | `slot` 是**回收槽位号**，不是节点 id；两个 list 的 slot 空间互不相干 | core 与 JS / 组件 |
| 一个 slot 同时只属于一个 item | `by_index` 保证不重复 bind | `ListState::update_window` |
| cell 必须在 `Op::Insert` 前/同时带 `Prop::Slot` | 否则 `add_slot_cell` 绑不上，cell 会变成"有节点没槽位" | `Core::apply` 的 `Prop::Slot` 分支、`Op::Insert` 分支 |
| cell 的 `parent` 必须是 list | `Prop::CellIndex` 从 `node.parent` 找 list，找不到就静默丢弃 | `Core::apply` |
| 行内容就绪后才发 `cellIndex` | `cellIndex` 是"我已经渲染成第 i 行了"的确认，早发会把空 cell 当实测值 | `Slot::ready` |
| `(slot,index)` 重复投递必须幂等 | `list::bind` 先 `peek_number` 再决定要不要 `set_signal`，但 `cellIndex` 每次都设 | JS 侧 |
| 滚动上报带 `applied` | `applied` = 平台已应用的 core 滚动条数（`ScrollBy` item + `ScrollTo` command） | `ListState::reported` 的 pending 重放 |
| 批量内 `ScrollBy` 与 `ScrollContent` 的相对顺序 | 内容**变长**时 `ScrollBy` 插在 list 的 `Frame` 之前；变**短**时排在 `ScrollContent` 之前 | 平台会按内容长度 clamp offset |
| 空的 `MountBatch` 返回 `None` | 纯 `LayoutAnimation` 的批量也算空 | 见 `docs/flows/frame.md` §5-4 |

## 4. 分支矩阵

| 维度 | 取值 | 走哪条路 | 备注 |
|---|---|---|---|
| 谁拥有行 | JS `<List>`（`Tag::List` + `ListData`） | `take_list_binds` → `Script::bind_list` → `list::bind` | 需要 engine 外层轮 |
| 谁拥有行 | Rust 组件 `virtual_list(count, row)`（`ElKind::VirtualList`） | `bind_virtual_lists` → `bind_virtual_cell` | 在 core 内层轮收敛 |
| 有无 JS | `script: Option<&Script>` 为 `None` | `drop(binds)`，JS 的列表**永远不出 cell** | 组件列表不受影响 |
| 轴 | `horizontal` | `update_lists` 里 `main`/`cross` 互换；`to_content` 用宽度 | 窗口、锚定、`ScrollBy` 的 dx/dy 都跟着换 |
| 方向 | `inverted` | `natural_offset` 从尾部镜像；`anchor_end` 保持距尾距离 | 锚定不变量在这种列表里换成"距尾不变" |
| 多列 | `columns > 1` | `row_of` / `row_size` 按行聚合，一行取最大项 | 与 `sticky` 互斥（`sticky_enabled` 要求单列且非 inverted） |
| 表头/表尾/空态 | `ListState::extras`（`ExtraKind::Header` / `Footer` / `Empty`） | 走 `measured_extra` + `full_width` + `set_extra` | **永不回收** |
| 是否需要重布局 | `relayout` 为 false | 只重算窗口，不跑 `layout_cells` | 单元格内的改动不动主树 |
| 跨轴变化 | `cells_cross` 变 | `layout_cells` 强制重排**全部** cell | 旋转/列数变化时是一次测量尖峰 |
| 布局失败 | `ApplyError::is_fatal` | `discard_ops` + `script.stop` | 非致命只 log，树保持一致 |
| 轮数耗尽 | 内层 8 轮 / 外层 8 轮 | 剩余 bind 留在队列里等下一次 commit | 见 §7-1 |
| 数据清空 | `count == 0` 或 `viewport <= 0` | `update_window` 把所有 slot `unbind` | 空态 cell 接位 |

## 5. 不变量

| # | 阶段边界 | 不变量 | 谁在依赖 |
|---|---|---|---|
| 1 | ⑨ 定窗 | 渲染窗口 = 视口 ± 1 屏 overscan（`OVERSCAN`），即约 3 屏；sticky 项额外保留 | `render_window`、`items_in`、`active_sticky` |
| 2 | ⑨ 测量 | 只有 `Slot::ready` 的 cell 参与测量/定位；`index != rendered` 的 cell 视同不存在 | `update_lists` 的两处 `ready` 收集 |
| 3 | ⑩ 锚定 | 视口以上的行改高（非 inverted，且整行都在视口之上）必须产生等量 `ScrollBy`，可见内容不动 | `ListState::measured` 的返回值 → `update_lists` 的 `anchor` |
| 4 | ① 进 commit | core 的 `offset` 是"平台 offset + 尚未应用的滚动"重放出来的；**过期的上报不能把窗口往回推** | `ListState::reported` 的 `pending` |
| 5 | ⑤ 复用 | 回收不改 cell 的节点 id，只改它显示的行；未确认前它显示的仍是上一行的内容 | `Slot::rendered` |
| 6 | ⑩ 摆放 | 没有 item 的 cell 若落在渲染窗口内（数据变短 / 跳转），必须被搬到 `-HIDDEN`；从未摆放过的（NaN frame）跳过 | 防"幽灵行" |
| 7 | ⑥ 确认 | `scrollToIndex` / `scrollToEnd` 的目标在"已到达且视口内所有行都已实测"之前一直 pin 着；用户滚走或 splice 才解 pin | `ListState::follow_pin`、`ListState::pin` |
| 8 | 整帧 | 一次 `turn` 只产出一条 `MountBatch`；多轮结果合并，不重排 | 继承 `docs/flows/frame.md` §5-2 |
| 9 | 回收 | 行内状态会**跟着 cell 走**（不跟行走）；跨行保持的状态必须放在列表外、按 id 键控 | 应用代码 |

> 🔴 第 3 条是"列表跳动"类 bug 的唯一判据：只要有一行在视口上方改了高度而这一帧没有对应 `ScrollBy`，画面必然跳。`crates/nsh-core/src/core/tests.rs` 的 `list_keeps_visible_content_anchored` 就是断言这一条的。

## 6. 缓存与记忆化点

| 缓存 | 位置 | 缓存键必须覆盖 | 失效条件 |
|---|---|---|---|
| 行尺寸 | `ListState::sizes`（每 item 一个 f32，NaN = 未测） | item 下标 | `splice` 插入段写 NaN；`set_count` 扩容写 NaN |
| 前缀和 | `ListState::rows`（Fenwick） | 行号 | `rebuild_offsets`（行数 / 列数 / 估计值 / splice / extras 变化） |
| 估计行高 | `ListState::estimated` | 每个 list 一份 | `set_estimated` 变值即重建 |
| 跨轴宽度 | `ListState::cells_cross` | list 的 cross 尺寸 | 变值 → 全部 cell 重排 |
| 待绑队列 | `Core::list_binds` | —— | `bind_virtual_lists` 取走组件的、`take_list_binds` 取走其余的 |
| sticky 结果 | `ListState::sent_sticky` | 计算出的 range 列表 | 与上次不等才发 `MountItem::StickyCells` |
| viewable | `ListState::viewable` | 计算出的 `(first,last)` | 与上次不等才发事件 |
| endReached | `ListState::end_reached_at` | 行数 | count 变 / splice / 滚离阈值 |
| 文本度量 | `MeasureCache` | 见 `docs/flows/frame.md` §6 | `fonts_changed` |
| JS 侧的 cell | `ListData::cells`（`Vec<Option<Cell>>`） | slot | list 销毁时 `dispose` 各 cell 的 owner |

> 🔴 **尺寸缓存的键是"下标"，不是"行 id"。** 所以 `Prop::Splice` 的 `(at, removed, inserted)` 只要算错一个，行高就会串到别的行上——而界面不会报错，只会"看着有点怪"。JS 侧的 `splice_run`（首尾相同前缀/后缀法）与组件侧的 `virtual_list::splice` 是同一套语义，改一边必须改另一边。

## 7. 未验证清单

| # | 问题 | 现状 | 怎么验证 |
|---|---|---|---|
| 1 | 外层 8 轮用尽后剩的 bind 靠谁唤醒 | **推断**：`Core::wants_frame` 只看组件任务（`components.services.frames`），`State::request_frame` 不看 `self.binds`。若这一轮又没发出滚动，就没有下一条消息 → 剩余 cell 要等下一次输入才渲染 | 在 `State::commit` 循环出口统计 `self.binds.len()`，跑一个"跳到未测量深处"的 gallery 场景 |
| 2 | 内层多跑的 `commit_round` 会不会让 JS 列表在**同一帧内**落后若干轮 | **推断**会：JS 的 bind 在整个 `Core::commit` 期间都留在队列里 | 同上，统计每次 `take_list_binds` 的条数与内层轮数 |
| 3 | `MAX_VIRTUAL_ROUNDS` 8 是否足够覆盖"一屏多行 + 多列 + sticky"的最坏情况 | **未查证** | 加计数后跑 gallery 的 sticky/多列示例 |
| 4 | 平台旁路 `Core::commit`（iOS `transition`、iOS/macOS `quiet`）产生的 bind 是否会被拖延 | **推断**会被拖延到下一次 `turn`（旁路不走 `take_list_binds`） | 在 `take_list_binds` 打点，跑转场 |
| 5 | `HIDDEN` 的幽灵 cell 在真机上是否真的不可见 | **未查证**：只有 core 单测，没有平台截图 | 平台侧加断言 / 截图对比 |
| 6 | 图里 `Core::commit` 的入边数与本图 `docs/flows/frame.md` 记的 148 不一致（本次查询 3 条） | **未查证**：可能索引版本/深度不同 | 用同一 query 参数各跑一次，记版本号 |

## 8. 可观测探针

| 想知道什么 | 在哪打点 | 怎么读 |
|---|---|---|
| 单帧 list 轮数 | `State::commit` 的 `MAX_LIST_ROUNDS` 循环自增计数 | `nsh::perf` 日志或临时 `eprintln` |
| 内层轮数 | `Core::commit` 的 `MAX_VIRTUAL_ROUNDS` 循环 | 同上 |
| 每帧 bind 条数 | `Core::take_list_binds` 入参 | 一次 `out.len()` |
| 是否有帧根本不产 bind | `State::commit` 的 `merged.is_some()` | 下表的 benchmark 已按"重绑帧/总帧"统计 |
| 锚定是否生效 | `update_lists` 里 `anchor != 0.0` 的次数 | 与视觉抖动对齐 |
| 帧成本 | `nsh::perf` 的 `commit: N ops, Tµs (list js Nµs), M mount items` | `nsh trace` |

实测（2026-10-21，Apple Silicon，本 worktree）：

```bash
cargo test --release -p nsh-core --lib list
# 39 passed; 0 failed —— 含 list_renders_window_and_recycles_cells（断言 400pt 视口 + 50pt 估计 = 17 个 cell，
# 实测 100pt 后内容长度 17*100 + 983*50）、list_keeps_visible_content_anchored（断言 ScrollBy 之和 > 0）

cargo test --release -p nsh-engine --test engine bench_gallery_scroll -- --ignored --nocapture
# back to back: 211 rebinding frames, 26.265µs/frame (js 11.299µs)
# sleep 10ms:   211 rebinding frames, 85.957µs/frame (js 43.43µs)
# evict caches: 211 rebinding frames, 108.09µs/frame (js 55.856µs)
```

`[实测]` 两条结论：2000 帧里只有 **211 帧真的重绑**（约 10.5%，滚 8pt/帧，overscan 边界约每 10 帧才跨一次行）；重绑帧的 JS 成本约 11µs（热）到 56µs（缓存冷）。

---

## 维护

改这条流程的 PR **必须同时改这张图**。改完跑：

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/lists.md .
```

命中率 < 90% = 这张图不可信，先修再用。

**不要往这张图里加的东西**：`MountItem` 每个变体的字段（看 `crates/nsh-core/src/mount.rs`）、`update_lists` 里每一步的公式（看 `crates/nsh-core/src/core/list.rs`）、行号。

---

## 🔴 图的覆盖边界（本次实测）

| 查询 | 结果 | 说明 |
|---|---|---|
| `Core::update_lists` outbound | `callees_total: 0` | 但它内部调了 `blocks` / `measured` / `update_window` / `set_frame` 等十余个函数 `[读码]` |
| `Core::commit` outbound | 0 | 实际调 `commit_round`、`bind_virtual_lists`，并 `std::mem::take` 取走 `self.mount` |
| `State::commit` callers | 2（`run_app` + 文件级节点） | 真实调用者 `Driver::turn` 不在其中 |
| `nsh-js` 的 `list::bind` inbound | 32 条，**全是同名误命中**（gpui 的 net 测试） | 名字太泛，图按名字回连，别信 |
| `Core::take_list_binds` | 2 条（方法 + `Core` 结构体） | 只能定位，不能追调用者 |

**结论：这张图里所有跨 crate 的调用边都是读码得来的；图只用来确认"符号存在且拼写对"。**

用图之前先做这一步：

```bash
codebase-memory-mcp cli --quiet trace_path \
  '{"project":"Users-hxy-Documents-Github-nsh","function_name":"<限定名>","direction":"outbound","depth":1}'
# callees_total > 0  → 图可用
# callees_total == 0 → 不能判定为叶子，退回读码
```
