# Flow · layout (invalidate → measure → place)

> Read this before touching `Core::commit_round`, before "why is this a frame behind",
> and before believing a test named "skips layout".
> Baseline `57b81b5e` (2026-10-21) · last checked 2026-10-21 · confidence:
> confirmed 20 / inferred 3 / gaps 6.
>
> Every claim is tagged `[图]` (codebase-memory), `[读码]` (source read) or `[实测]`
> (command run, given). Line numbers are deliberately absent: they rot.

---

## 1. 触发点

Layout is invalidated through **one global flag** (`needs_layout`) plus **per-node taffy
dirty bits**. Every setter below sets `needs_layout`; the per-node bit is what decides
*which* tree actually gets walked.

| # | Trigger | Entry symbol | File | Note |
|---|---|---|---|---|
| 1 | Any style push for a node that has a taffy node | `sync_layout_style` / `set_style` `[读码]` | `crates/nsh-core/src/core/mod.rs` | **Includes paint-only props** (`backgroundColor`, `opacity`, `transform`); see §4 |
| 2 | Text content or metrics changed | `rebuild_text` → `mark_dirty` `[读码]` | same | Only when `layout_eq` is false; colors are excluded |
| 3 | Class / state restyle | `restyle` `[读码]` | `crates/nsh-core/src/core/classes.rs` | Ends in the same `set_style`, so a hover color change relayouts |
| 4 | Insert / remove / replace of a node | `insert` / `remove` / the layout-node swap `[读码]` | `crates/nsh-core/src/core/mod.rs` | Also decides whether the child joins the parent's taffy tree (§2) |
| 5 | Input text (value/placeholder) | `measure_input` `[读码]` | same | Rebuilds the measured text when `layout_eq` says it changed |
| 6 | Window resize | `set_viewport` `[读码]` | same | Re-pushes the root style, restyles `WIDTH`/`HEIGHT` class deps |
| 7 | Safe area / header bar height / measured screen header | `set_safe_area` / `set_header_heights` / `set_screen_header` `[读码]` | same | Screens lay their content out below the header |
| 8 | Modal reports its size / insets | `set_root_size` `[读码]` | same | Modals are their own layout roots |
| 9 | Native component reports its size | `set_component_size` `[读码]` | same | Only when the size differs |
| 10 | Image source natural size | `set_style` image branch `[读码]` | same | `LeafContext::Image(natural)` feeds the measure closure |
| 11 | Fonts changed | `fonts_changed` `[读码]` | same | Drops the whole `MeasureCache`, dirties every text block |
| 12 | Screen/modal options | the `ScreenProp` branch `[读码]` | same | Clears `RootState.laid_out` (header changes move the content) |
| 13 | First commit | `needs_layout: true` in `with_engine` `[读码]` | same | The initial tree is never laid out |

**Not a trigger** `[读码]`: `set_scroll_offset`, `set_zoom`, `set_frame`. Scrolling only reaches
layout through the cells the list asks for in `update_lists` (§2).

Callers: `commit_round` is the only caller of the layout phase ③–⑧, and `commit` is the
only caller of `commit_round` `[读码]`. The graph knows about three callers of `commit` —
the platform bypasses in `docs/flows/frame.md` §1 — and **misses** the engine's
`State::commit`, which calls it once per bind round `[图+读码]`. Do not read that as
"platforms drive layout".

## 2. 阶段链

| Stage | Symbols | File | In | Out |
|---|---|---|---|---|
| ① 脏标记处理 | `flush_dirty_once` (looped by `flush_dirty`) → `rebuild_text` `[读码]` | `crates/nsh-core/src/core/mod.rs` | the `dirty` bits queued in `queue` | text rebuilt, items queued, `needs_layout` possibly set |
| ② 门 | `needs_layout` read + cleared `[读码]` | same | the flag | `relayout: bool` for this round |
| ③ 主树 | `layout_main` → taffy compute `[读码]` | same | viewport as the definite available space | taffy layout for the surface tree; `bool` = "was laid out again" |
| ④ 帧回写（主树） | `emit_frames` `[读码]` | same | the fresh taffy layout | `MountItem::Frame` for changed frames only |
| ⑤ 独立根 | `layout_roots` → `layout_detached` `[读码]` | same | screens, modals, header items, input accessories | per-root size+header memo, frames, `ScreenConfig` |
| ⑥ 列表单元 | `layout_cells` `[读码]` | same | lists and their cells | cell layout at the list's cross size |
| ⑦ 列表定位 | `update_lists` `[读码]` | same + `crates/nsh-core/src/core/list.rs` | `relayout`, `list.dirty`, commands | cell frames, `ScrollContent`, binds for JS |
| ⑧ 文本终局 | `flush_text` → `place_attachments` `[读码]` | same | text content boxes from ③/⑥ | `MountItem::Text`, attachment frames |

`commit_round` runs ①→⑧ in that order, once; `commit` runs `commit_round`, then up to
`MAX_VIRTUAL_ROUNDS` extra rounds while `bind_virtual_lists` keeps binding `[读码]`.

```
invalidate (any §1 trigger)
   │  needs_layout = true
   │  taffy mark_dirty(node)      ← per tree, what actually gates ③/⑤/⑥
   │  node.dirty |= …             ← what flush_dirty consumes
   ▼
commit_round
   ├─ render_components / sync_back / sync_keys
   ├─ flush_dirty ─ rebuild_text ─ layout_eq? ── no ─→ mark_dirty + needs_layout
   │                                    └────── yes ─→ measurement cache kept
   ├─ relayout = needs_layout ; needs_layout = false      ← the gate, per round
   ├─ layout_main: layout.dirty(root)?
   │        ├─ false → nothing (no compute, no frame walk)
   │        └─ true  → layout_root(root, viewport)
   │                     └─ emit_frames(each child)        ← only changed frames
   ├─ layout_roots: for each detached root, outer first:
   │        (size, header) != laid_out  ||  layout.dirty(root)
   ├─ layout_cells: for each list:
   │        cells_cross changed → all cells; else only dirty cells
   ├─ update_lists(relayout) ── cell frames, ScrollContent, binds
   └─ flush_text ────────────── MountItem::Text, place_attachments
```

## 3. 数据契约

| Contract | Must hold | Who depends on it |
|---|---|---|
| `node.frame` is **parent-relative** | platforms call `setFrame` with it as-is; the core mirrors it for RTL scroll parents before sending | `crates/nsh-platform-macos/src/view.rs`, `emit_frames` |
| A frame is emitted **only when it changed** | an unchanged relayout produces zero items | every platform; the "color change" test relies on it |
| `emit_frames` walks **parents before children**, in document order | the platform can place a child after its parent is placed | platform mounters |
| Cells are **never** in the main taffy tree | a cell change cannot dirty the surface root | the `dirty(root)` gate in `layout_main` |
| The main-axis position of a cell comes from `update_lists`, not taffy | taffy only gives the cell its cross size and height | `list.rs` windowing, sticky, anchors |
| A cell without an item is moved out of sight at `-HIDDEN` (1e6) when it would show through | it keeps its content but must not be visible nor hit-testable | `update_lists` |
| Text block frame from taffy; text **content box** from `content_box(...)` | padding/border insets, plus `align_vertical` and `no_wrap` overrides | `flush_text`, `MountItem::Text` |
| Attachment views are placed by the text layout, not by taffy | their frames are overwritten by `place_attachments` after `flush_text` | text attachments |
| `layout_main` returns "the main tree was laid out again" | callers must not emit main-tree frames otherwise | `commit_round` |
| Leaves report **no intrinsic size** | `compute_leaf_layout` gets a zero content-size function; only the measured size is used | flex sizing of text/image/native leaves `[读码]` |

## 4. 分支矩阵

### 4.1 Which tree is walked

| Dimension | Value | Path | Note |
|---|---|---|---|
| Global flag | `needs_layout == false` | nothing at all | measured: 202 / 1049 commits `[实测]` |
| Main tree | `layout.dirty(root) == false` | `layout_main` returns false, **no frame walk** | a change inside a cell lands here |
| Main tree | dirty | `layout_root` + `emit_frames` | also the path for a paint-only change (§4.2) |
| Detached root | `(size, header)` unchanged and clean | skipped | `RootState.laid_out` memo |
| Cell | `cells_cross` changed | **all** cells relaid (`resized`) | the list's own width/height changed |
| Cell | cell clean | skipped | one row edit does not relayout the other rows |
| Node kind | `Kind::Cell`, `Kind::Screen`, `Kind::Modal`, … | its own taffy root | `Kind::is_root`, `Kind::derives_layout` |

### 4.2 What a "style-only" change actually costs `[实测]`

One temporary unit test, one `Op::SetStyle` per row, on a tree of `[view, text block
(raw "a" + span "b")]`. "Layout-relevant text change" is whether `rebuild_text` saw a
different `layout_hash` (its own `layout_eq` check).

| Change | Main-tree layout | Text rebuild saw a metric change |
|---|---|---|
| `backgroundColor` on a `View` | **yes** (1) | – |
| `opacity` / `transform` on a `View` | **yes** (1) | – |
| `color` on a **text block** | **yes** (1) | no |
| `color` on an **inline span**, run structure unchanged | **no** (0) | no |
| `backgroundColor` on an inline span | **no** (0) | no |
| `color` on a span that **splits a run** (the span's first color) | **yes** (1) | yes (run count 1 → 2) |
| `fontSize` on a span | **yes** (1) | yes |
| a **text** change inside a **list cell** | **no** (0) | yes |

Two mechanisms explain every row:

* A node with a taffy node gets `set_style` called on it for **any** style field, and
  taffy's `set_style` always marks the node **and its ancestors** dirty `[读码]`
  (`taffy` 0.15, `set_style` in its tree implementation). Paint-only props are not filtered
  out on the way down. So "paint only" does not mean "no layout".
* An inline span has **no taffy node at all** — `set_inline` removes it `[读码]` — so
  `sync_layout_style` returns early on its missing layout, and if `layout_eq` also says the
  runs did not change, nothing invalidates either tree.

> The test name `color_change_skips_layout_and_identical_texts_share_measurements`
> (`crates/nsh-core/src/core/tests.rs`) is about **observable** work: no re-measure, and a
> batch with a single `Text` item. A full main-tree layout still runs `[实测]`. Do not use
> that test as evidence that a color change is free.

### 4.3 Text measurement inside layout

| Situation | `max_width` passed to the measurement | Consequence |
|---|---|---|
| `no_wrap` | `None` | natural width; the content box may be narrower (text overflows) |
| Parent gave a definite width | that width | wraps at it |
| Available space is `MinContent` / `MaxContent` | `None` | natural width; a text has no cheap min-content `[读码]` |

Text is measured through `measure_text`, keyed by `(layout_hash, max_width)` in the shared
`MeasureCache` `[读码]`. Identical content in three blocks is measured once per distinct
constraint `[实测]` (the same test asserts ≤ 2 measures and 3 final layouts).

## 5. 不变量

| # | Boundary | Invariant | Who relies on it |
|---|---|---|---|
| 1 | ② | `needs_layout` is cleared **before** the layout runs, so anything that re-dirties during ③–⑧ lands in the *next* round, not this one | one layout per round |
| 2 | ③ | At most **one** `layout_root(root)` per `commit_round` | cost bound |
| 3 | ③ | A list cell never dirties the surface root | §4.1; `[读码+实测]` |
| 4 | ④/⑤ | Frames are emitted only for changes, parents first | platforms diff-free apply |
| 5 | ⑥ | A cell is laid out at exactly the list's cross size for its slot kind | column layouts |
| 6 | ⑧ | A text block's final layout is produced **after** its box is final | `onTextLayout`, attachments |
| 7 | ⑧ | Identical texts share measurements; the key must cover every metric input | §6 |
| 8 | ⑧ | `prepared` is dropped on **any** text change, `layout_hash` only on metric changes | platform typesetting state embeds color |

**A frame is not a round.** `commit` may run `commit_round` up to
`1 + MAX_VIRTUAL_ROUNDS`, and the engine's frame loop calls `commit` up to
`MAX_LIST_ROUNDS` times `[读码]` (`crates/nsh-engine/src/lib.rs`). So the true bound is
**one full main-tree layout per round**; the frame-level bound is
"one per round that dirtied the main tree", not "one per frame". Measured per *commit*
over the whole `nsh-core` test suite (333 tests, 1049 commits): `rounds` 1 → 1014,
2 → 24, never more; `full layouts` 0 → 202, 1 → 843, **never 2** `[实测]`.

## 6. 缓存点

| Cache | Where | Key must cover | Invalidated by |
|---|---|---|---|
| Text measurement | `MeasureCache` (`crates/nsh-core/src/text.rs`) | `layout_hash` (content + runs + metrics) + `max_width` bits, `NAN` = unconstrained | `fonts_changed`; 2 generations × 512, LRU-ish |
| Per-node last measure | `TextContext::cache` `[读码]` | that node's `(max_width, size)` | text rebuild with a metric change |
| Platform typesetting | `TextContext::prepared` `[读码]` | the full `AttributedText` **including colors** | every text rebuild |
| Final text layout | `TextContext::laid_out` `[读码]` | `(content box, TextLayout)` | any text rebuild or a new box |
| Layout identity of text | `AttributedText::layout_eq` / `layout_hash` `[读码]` | text, runs, align, wrap, max lines, ellipsize, attachments | — |
| Taffy layout | the tree itself, per-node dirty bits | style push + measure invalidation | `compute_layout_with_measure` |
| List cross size | `ListState::cells_cross` `[读码]` | the cross size the cells were laid out at | a real resize relayouts **all** cells |
| Detached root size | `RootState::laid_out` `[读码]` | `(size, header)` | any screen-option change |
| Cell position | `node.frame` (`set_frame`) `[读码]` | last sent frame; also hit-testing and `Core::measure` | layout, `update_lists` |

> `layout_hash` **deliberately** ignores color, so the cache survives recolors; `prepared`
> must **not**, because CoreText/StaticLayout bake color into the typesetting state. Getting
> these two the same way round is the whole trick of §4.2.

## 7. 未验证清单

| # | Question | Status | How to check |
|---|---|---|---|
| 1 | Does the engine's 8-round frame loop ever produce **two** full layouts in one frame? | **unverified**. Structurally possible (each `commit` re-reads `needs_layout`), never seen per commit | add the §8 counters and log per round index in `State::commit`; run `cargo test --release -p nsh-engine --test engine bench_gallery_scroll -- --ignored --nocapture` |
| 2 | Can a JS re-render in round *n+1* dirty the main tree (e.g. `onLayout` → state → container style)? | **inferred possible**; the Rust-component analogue measured 0 (`Chat` press → 0 full layouts) | same probe, with a gallery screen that reacts to `onLayout` |
| 3 | `docs/flows/frame.md` §5 item 3 says "同一帧内不会重复全树布局" | **wrong as stated**; correct per round/commit (§5) | this doc's §5 measurements |
| 4 | Does a span color change keep the run count when the block has other same-colored text? | **unverified** — my probe deliberately used a single span; run merging may hide the split | extend the §8 probe to a 3-span block |
| 5 | When `cells_cross` changes, is "relayout all cells" bounded in practice (2000-row list)? | **unverified**; the loop is O(cells in the window), but the window size after a cross change is untested | `bench_cell_relayout` with a viewport-width change per frame |
| 6 | Attachment children: when only the *text* changes and the attachment's own layout does not, are the attachment's descendants' frames re-emitted? | **unverified**; `place_attachments` sets the attachment frame but does not call `emit_frames` on its subtree | probe with a text block holding a nested view attachment whose inner text changes |

## 8. 可观测探针

All numbers in this doc came from three counters added temporarily next to
`needs_layout` and printed from `commit` when NSH_LAYOUT_PROBE is set. They were
reverted before committing; re-add them when you need to answer "how many layouts".

| Want to know | Instrument | Read |
|---|---|---|
| Rounds per commit | `commit_round` entry +1 | `rounds=` |
| Full main-tree layouts | `layout_main` `if main` +1 | `full_layouts=` |
| Cell layouts | `layout_cells` `if resized \|\| dirty` +1 | `cell_layouts=` |
| Why `needs_layout` turned on | print `line!()` after every assignment | the site in §1 |

```bash
# 1 full layout per commit, 1 round: a plain row with two texts
cargo test --release -p nsh-core bench_row_relayout -- --ignored --nocapture
# 0 full layouts, 1 cell layout per commit: the same row inside a list cell
cargo test --release -p nsh-core bench_cell_relayout -- --ignored --nocapture
# 2 rounds on the first commit (bind + settle), then 0 full layouts while scrolling a
# virtual list: a temporary test mounting a component with `virtual_list`, scrolling it
cargo test -p nsh-core <the probe> -- --nocapture
```

Measured (Apple Silicon, release for the benches):

| Run | Commits | Rounds | Full layouts | Cell layouts | Time |
|---|---|---|---|---|---|
| `bench_row_relayout` | 2001 | 1 | 1 each | 0 | 6.2 µs/commit |
| `bench_cell_relayout` | 2102 | 1 | 1 (first), 0 (rest) | 0, then 1 each | 5.8 µs hot / 45.6 µs after 10 ms idle |
| whole `nsh-core` suite | 1049 | 1: 1014, 2: 24 | 0: 202, 1: 843 | — | — |

---

## 维护

Changing the layout path (adding an invalidation source, a new taffy tree, a new cache)
means changing this map in the same PR. Then:

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/layout.md .
```

Below 90% symbol hits = this map is stale; fix it before using it.

**Do not add here**: call chains the graph can compute, line numbers, the full
`MountItem` list (see `crates/nsh-core/src/mount.rs`), or platform-specific placement
details (see `docs/flows/frame.md`).

---

## Appendix · Why this doc is almost entirely `[读码]`

The graph has **no usable edges** for any symbol of this flow. Queried with the exact
qualified name from the graph's search tool:

```
Core.layout_main      callees_total: 0   callers: __file__ only
Core.layout_cells     callees_total: 0   callers: __file__ only
Core.layout_roots     callees_total: 0   callers: __file__ only
Core.layout_detached  callees_total: 0   callers: __file__ only
Core.layout_root      callees_total: 0   callers: __file__ only
Core.commit_round     callees_total: 0   callers: __file__ only
Core.flush_dirty      callees_total: 0   callers: __file__ only
Core.emit_frames      callees_total: 0   callers: __file__ only
Core.update_lists     callees_total: 0   callers: __file__ only
Core.commit           callees_total: 0   callers: 3 (platform bypasses, see frame.md §1)
```

`commit_round` calls eight of the functions above, so `callees_total: 0` is **not** "leaf" —
it is the known 15% coverage of `nsh-core` (`docs/flows/frame.md`, appendix). Use the graph
here only to find definitions, never to reconstruct order.
