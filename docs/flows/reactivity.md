# Flow · TSX reactivity (a signal write → who re-runs)

> Read this before changing an insert, a memo, `<If>` / `<Each>`, or a component's children.
> It answers one question: **a setter was called — which computations run, in what order, and what
> gets *remade* instead of updated.** Same eight sections as `frame.md`, so the two read side by
> side: this file ends where the frame's §1 begins.
> Upstream shape read at upstream/main `5fe6c614` (after #174 and #216) · this worktree `57b81b5e`
> (before both) · last checked 2026-10-21 · confidence: confirmed 21 / inferred 4 / open 6

**Two trees, one map.** `164415ce` (#174, `insert_items`) and `13fdeb64` (#216, `item_memo` /
`item_run`) are on upstream/main and **not** in this worktree: here `insert_value` still does
everything in one effect, so both bug shapes in §4.1 are live in this checkout. Facts marked
`git show <sha>` were read from the upstream blob, not from a file you can grep here.

Source tags: `[图]` = `codebase-memory-mcp` (project `Users-hxy-Documents-Github-nsh`, indexed at
`57b81b5e`) · `[读码]` = read from source · `[实测]` = a command that was run, quoted.
**No behavioural run was done here**: this worktree has no build cache, and the main workspace's is
out of bounds. Every claim below is `[图]` or `[读码]`; §7 says what would confirm each one.

---

## 1. Trigger points (all of them)

A flush starts at a **write**, not at a frame. Nothing in this path batches by frame: the ops a
write produces are queued and taken by the next `Core::commit` (see `frame.md`).

| # | Trigger | Entry symbol | File | Note |
|---|---|---|---|---|
| 1 | `count.set(v)` | `signal_set` → `write` [读码] | `crates/nsh-js/src/dom.rs` | the common one |
| 2 | `count.update(f)` | `signal_update` → `write` [读码] | `crates/nsh-js/src/dom.rs` | `f(prev)` runs before the write, tracked by nothing |
| 3 | Rust writes a signal the renderer owns | `set_signal` → `write` [读码] | `crates/nsh-js/src/dom.rs` | list rows (`each_run`), the device module's signals |
| 4 | A memo writes its new value | `write` from `run_computation` [图] | `crates/nsh-core/src/reactive.rs` | a memo is a writer *and* a reader: it can start a round while one is running |
| 5 | Many writes, one round | `batch` → `run_updates` [图] | `crates/nsh-core/src/reactive.rs` | the outer round is joined, not nested |
| 6 | A computation is **made** | `memo` / `effect` → `update_computation` [读码] | `crates/nsh-core/src/reactive.rs` | runs at once, **inside a round too** — the only way to re-enter the scheduler without a write |
| 7 | A round ended with an error | `finish` [读码] | `crates/nsh-js/src/reactive.rs` | the first error of a round is thrown from the write that started it, after the round |

> 🔴 **A setter is not the only writer.** `write` also runs from inside `run_computation` (§2 ③) and
> from the memo `cond` makes. So "one setter → one round" holds, but "one round → one setter" does
> not: a round can run several `complete_updates` passes (§4.3).

## 2. Stage chain

| Stage | Key symbols | File | In | Out |
|---|---|---|---|---|
| ① Change test | `changed` [图] | `crates/nsh-core/src/reactive.rs` | old value, new value, `Equals` | run or return |
| ② Mark + queue | `write` → `queue_update` / `queue_effect` / `mark_downstream` [图] | same | the signal's observer edges | `updates` (memos) and `effects` (effects) queues |
| ③ One round | `run_updates` → `complete_updates` [图] | same | the two queues | rounds until no effect is queued |
| ④ Memo queue | `run_queued_updates` → `run_top` → `update_computation` → `run_computation` [图] | same | `updates` | new values, more queues |
| ⑤ Pull upstream | `look_upstream` [图] | same | a PENDING node | its stale memos brought up to date first |
| ⑥ Effect stage | `run_effects` [图] | same | `effects` | render effects, then user effects |
| ⑦ Renderer side | Effect bodies `insert_run` / `bind_prop_run` / `each_run`, memo bodies `cond_run` / `each` [读码] | `crates/nsh-js/src/dom.rs` | the new values | `Op`s pushed to the host queue |
| ⑧ Graph → device | `Op` → `Core::apply` [图] | `crates/nsh-core/src/ops.rs` | ops | one mount batch (`frame.md` §2) |

Stage ⑥ is where TSX code meets the graph: an effect body is `insert_run` (children), `bind_prop_run`
(one prop), `each_run` (a list's rows), or the memos of `each` / `cond`. The scheduler knows nothing
about views and the renderer knows nothing about rounds. Everything interesting is in **which node is
the listener of which call**.

### 2.1 What one setter schedules

Canonical shape — a component passing its children through, one sibling that changes:

```tsx
function Wrap(props: { children?: any }) { return <View>{props.children}</View>; }

<View>
  <Wrap>
    <Input testID="a" />
    <If when={saved()}>{(() => <Input testID="b" />)()}</If>
  </Wrap>
  <Text onPress={() => saved.set(true)}>save</Text>
</View>
```

Compiled shape [读码 `crates/nsh-bundler/src/jsx.rs`; `git show 164415ce` for the children fix,
`git show 13fdeb64` for the branch fix] — the getter makes every sibling, `<If>` is a call of
`cond` (a memo), and the `<If>`'s expression branch is a function:

```text
component(Wrap, { get children() { return [element("input"), cond(() => saved(), () => (() => element("input")))]; } })

saved.set(true)
      │  signal_set → write
      │  changed()?  Truthiness / identity → yes
      ▼
┌─ write: the observers of `saved`, in edge order ───────────────────────────────┐
│  C1  condition memo  (reads saved(), Equals::Truthiness)  → updates queue       │
│  M2  item memo       (the ternary accessor)               → updates queue       │
│      mark_downstream(C1): C2 → updates, M1 → updates, E1 → effects              │
│      every one of them: state = STALE                                           │
└─────────────────────────────────────────────────────────────────────────────────┘
      ▼
  run_updates(init = false)                     ← one round, exec += 1
      ▼
  run_queued_updates   (memos, in queue order)
      C1  runs → value changed (truthiness) → writes → C2 STALE
      C2  runs → cond_run: reads C1, calls branch() UNTRACKED → returns the accessor
                → value changed (undefined → fn) → writes → M1 STALE
      M1  runs → item_run: calls C2's value → gets a function → makes M1a, a NEW memo
                → M1's value is a new object → writes → E1 STALE
      M2  runs → the ternary accessor → element() → a NEW node → writes → E1 STALE
      ▼
  complete_updates loop → run_effects:
      render effects first, then user effects
      E1  insert_items_run   the array's effect  ── exactly one run
      ▼
  E1's body: resolve() reads M1 (clean) → M1a (clean) → last time's input node
             reads M2  (clean) → the NEW input node
             reconcile() → Create + Insert for the new sibling only
```

The node / owner tree that diagram assumes [读码, `git show 164415ce` + `git show 13fdeb64`]:

```text
root  (mount → with_root)                       owns everything below unless noted
└── E0  insert_run        "the value of Wrap's <View> children"
    │     sources: what the children getter itself reads  ← post-#174: none of the items
    ├── E1  insert_items_run   "the array"        ← the ONE effect a child change runs
    │     sources: M1, M1a, M2  (memos only, never the functions behind them)
    ├── M1  item_memo(C2)        made by memoize_run, owned by E0
    │   └── M1a item_memo(accessor)  made by item_run, owned by M1
    ├── M2  item_memo(ternary)   made by memoize_run, owned by E0
    └── C1 condition ← C2 branch   made by cond(), owned by E0
```

> 🔴 **The boundary is the listener.** A call is tracked by whoever's `run_computation` is on the
> stack. Before #174, `insert_run` called the getter *and* the functions the getter returned: one
> listener for both, so an item change ran the getter, and the getter re-made every sibling.
> `insert_items` gives the array an effect of its own, which splits the two boundaries. `item_memo`
> gives a *returned* function a memo of its own, so the array's effect reads a value instead of
> calling a producer.

## 3. Data contracts

| Contract | Must hold | Who relies on it |
|---|---|---|
| `updates` and `effects` are two queues | memos run before the effects of the same round | `run_queued_updates` before `run_effects` |
| a node's `state` is CLEAN / STALE / PENDING | at most one run per node per round; PENDING pulls upstream instead of re-running | the two early returns in `run_top` |
| `updated_at` is an exec counter, not a time | nothing recomputes twice within one exec | the ancestor walk in `run_top`, `look_upstream` |
| a memo's value is compared **by identity** (`Equals::Default`) | the same object = unchanged; a new object = changed | every reader of a memo |
| a computation runs once when it is made | the code that made it can read the result at once | `insert_value`'s return, `render_effect` |
| the effects of one insert share `shown` (an array) | when the outer effect re-runs, the nested one reconciles against the **real previous nodes**, else nodes are inserted twice | `insert_run` → `insert_items` |
| an item memo's value may be a memo object | the array's effect keeps calling it, reading a cache instead of remaking nodes | `item_run`'s return value |
| one error per round, the first | thrown from the call that wrote; the graph stays usable | `JsData::error` + `finish` |

## 4. Branch matrix

### 4.1 The two bug shapes (#174 / #216)

| | Symptom | Mechanism | Fix |
|---|---|---|---|
| #174 `164415ce` | changing one child **remade all its siblings**. An `<Input>` lost its text and focus, a list beside it lost its scroll position. `<If>` / `<Each>` / ternaries alike; `Screen` and host elements' own children unaffected | when the insert's value is `() => props.children`, **one** effect both called the getter (depending on what the getter reads) and called the functions inside the array the getter returned (depending on each child's inner signals). A child changed → that effect re-ran → the getter re-ran → every sibling's `element(...)` ran again → new nodes | an array holding functions gets a render effect of its own (`insert_items`, Solid's nested effect in insertExpression); each function still gets a memo (`memoize_run`); both effects share `shown` |
| #174, second cause | a ternary written inside the getter: `["one ", s()]` | the compiler inlined the changing expression into `get children()`, so the getter itself read `s()` | the compiler: among more than one child, a changing expression becomes a function of its own, `["one ", () => s()]`; a lone child stays the getter's, `get children() { return s(); }` |
| #216 `13fdeb64` | when an `<If>`'s branch is an **expression** (`{(() => …)()}`), a sibling change re-ran it and remade its nodes (the gallery's `DemoPage` cards replayed their enter animation whenever a dialog beside them opened) | `cond_run` reads `condition` tracked and calls `branch()` untracked, and returns **a function**. The array's effect called that function → the effect depended on the branch's inner signals → any sibling change re-ran it | `item_run`: when an item's function returns a function, that function gets a memo of its own (item_memo) under the item's memo. The array's effect only reads memos |

**Before → after #174** (mount items for one tap, from the commit message): KeyboardAvoidingView +
Input + `<If>` 24 → 4; a fetch-driven list 226 → 6; the same idle 186 → 148; gallery first frame
937 / 1121 unchanged; counter 49 unchanged. **Before → after #216** (Form demo, open/close the
sheet): 1200×800 open 1887 → 266, close 1696 → 75; 390×844 unchanged at 266 / 75.

### 4.2 The four kinds of insert value

| Insert value | Path taken | Boundary (who is tracked) |
|---|---|---|
| static (no function, no array holding one) | `resolve` + `insert_before`, **no effect at all** | none; it cannot change |
| one function | one effect (`insert_run`) that calls functions until the value is not a function | **that effect is the boundary**: every signal read inside is its dependency. `MAX_DEPTH` = 64 |
| an array of ≥ 2 holding at least one function | `insert_items`: `memoize_run` gives each function a memo, then one more effect | the array's effect reads memos only; each memo is its own boundary |
| an array of 0 or 1, or an array with no function | `memoize_run` returns `None`; handled as a single value | as the two rows above |

> 🔴 **`items.len() < 2` means no item memo.** With **one** dynamic child in a host element
> (a `<View>` holding one `<If>` whose child is an expression), the accessor `<If>` returns is called
> by the insert's own effect: `s` changes → the effect re-runs → `element()` makes a new node
> (Remove + Create) instead of updating the existing one. That is an explicit condition in
> `memoize_run`, not an oversight of the fix; read §7-3 before changing it.

### 4.3 When it runs one extra time

| # | Shape | Mechanism | Verdict |
|---|---|---|---|
| 1 | the outer effect re-runs → **all N item memos are rebuilt and each runs at once** | `memoize_run` calls `reactive::memo` synchronously inside `insert_items`, and `memo` runs a computation when it makes it | N+2 runs, 0 mount items. **This is most of #174's 226 → 6**: before, the effect re-ran and every node was remade |
| 2 | an item's function returns a function, so the item memo's value is a **new memo object every run** → always "changed" | `Equals::Default` compares identity; `item_run` builds a fresh item_memo each run | intended (#216), but its readers always run once. "memo re-ran, nothing changed, 0 ops" is this one |
| 3 | one node is written twice in one round | the second `write` marks an already-CLEAN reader STALE again, and the next pass of `complete_updates` runs it | legitimate: two real changes. **But** if an effect writes a signal read by an effect that ran *earlier*, that earlier effect runs again in the next pass (order = edge creation order) |
| 4 | the getter reads something that changes (#174's second cause) | the compiler inlined the expression into the getter | a regression: one setter hits both the outer and the item boundary |
| 5 | a memo returns **new node objects** (`element()` called again) | identity differs → reconcile sees no shared pointer → Remove + Create | a regression: this is "the input lost focus" |
| 6 | `Equals::Never` (`equals: false`) or a custom comparison returning false | every write counts as a change | the caller's choice |
| 7 | one effect reads both `a()` and a memo of it | two edges, but the CLEAN early return in `run_top` collapses them into one run | not a duplicate |

**How to tell**: after one setter, a *memo* that ran twice is a defect; an *effect* that ran twice is
almost always being notified through two different memos (shape 1 / 2), or wrote a signal itself
(shape 3).

### 4.4 The remaining dimensions

| Dimension | Values | Path | Note |
|---|---|---|---|
| equality | `Equals::Default` / `Truthiness` / `Never` / `Custom` | four branches of `changed` | `<If>`'s `when` uses `Truthiness`: `0 → -1` does not re-run the branch |
| inside a round? | `updates.is_some()` | inside → writes only queue; outside → start a round | the first line of `run_updates` |
| making a user effect | inside a round / outside | inside → queued after this round's effects; outside → runs now | the branch in `effect` |
| owner disposed (`DISPOSED`) | yes / no | yes → what is made is disposed at once, cleanups run at once | a listener that closes the view it belongs to |
| a component | `component()` | **untracked**; the component body runs once | a component never re-runs on a signal; its own prop and insert effects do |
| a component's child count | 1 / > 1 | 1 → the expression stays in the getter; > 1 → each expression is a function | decided at compile time; drives §4.3-4 |
| an `<Each>` row | reused by key / new | a reused row only writes its `item` / `index` signals; a new row gets a new owner through `with_root_under` | effects inside the row follow `item()` |
| a list cell | recycled | a recycled cell only writes its `index`; `item_run` recomputes the item | **a different `item_run`** from the renderer's (§6) |

## 5. Invariants

| # | Boundary | Invariant | Who relies on it |
|---|---|---|---|
| 1 | within a round | a node runs at most once; CLEAN and PENDING both return early in `run_top` | every reader of a memo |
| 2 | within a round | memos before effects; render effects before user effects | `run_queued_updates` / `run_effects` |
| 3 | reading a memo | the value read is current (STALE computes in place, PENDING pulls upstream first) | glitch-free diamonds: one consistent set of values |
| 4 | change propagation | only along observer edges; **an ancestor never re-runs because a descendant changed** | the whole point of #174 |
| 5 | making a computation | it belongs to whoever was running; that run disposes it before running again | the `owned` loop in `clean_node` |
| 6 | the outer effect re-running | the old `insert_items` effect and its item memos are disposed first, then rebuilt against the **same** `shown` | no duplicate nodes |
| 7 | one setter | one error per round, thrown from the JS call that wrote; the graph stays usable | the app's own try/catch |
| 8 | ownership ≠ dependency | a component belongs to whoever made it; dependencies only decide who re-runs | "why was my component remade" |

> 🔴 Invariant 8 is the one people get wrong: **dependency decides whether it runs, ownership decides
> what is remade.** A component made inside an insert effect is owned by that effect, so when the
> effect re-runs the component and its state are gone — even though no dependency pointed at it.

## 6. Caches and memoization points

| Cache | Where | The key must cover | Invalidated by |
|---|---|---|---|
| memo | a node in `Graph` (`crates/nsh-core/src/reactive.rs`) | `(id, generation)` of every source it read | a source writing, or its owner re-running (`clean_node` unlinks, then re-reads) |
| item memo (one per function in an array) | `memoize_run` (`crates/nsh-js/src/dom.rs`) | the function object + that run of the outer effect | the outer effect re-running: all rebuilt |
| branch memo (an `<If>`'s expression branch) | `item_run` (`crates/nsh-js/src/dom.rs`, upstream #216) | the branch function + the signals it reads | the item memo re-running |
| `<If>`'s truthiness | `cond`'s `condition` memo, `Equals::Truthiness` | **truthiness**, not the value | truthiness flipping |
| list row item | `item_run` (`crates/nsh-js/src/list.rs`) — **same name, other thing** | the row's `index` signal + `props.each` | a cell recycled to another index |
| reused text node | `materialize`'s `reuse` | "old and new are both one text" | more than one node on either side |

> 🔴 Two `item_run`. The one in `crates/nsh-js/src/dom.rs` (upstream #216) wraps **one function of an
> array**; the one in `crates/nsh-js/src/list.rs` wraps **one list row's item**. Check the file
> before grepping.

**Slot lifetime**: a memo named by a JS handle (all of `item_memo`'s) **keeps its value and slot**
after being disposed, until the JS object is collected and `drop_handle` runs. Code that rebuilds
item memos often therefore keeps `Graph::slots()`'s used count high for a long time — observable,
see §8.

## 7. Unverified

| # | Question | State | How to verify |
|---|---|---|---|
| 1 | Is the schedule in §2.1 (six runs) the measured one | **inferred** from the early returns in `run_top` / `mark_downstream`; never counted | count in `run_computation` (or log it) and run `compiled::a_changing_child_leaves_its_siblings` |
| 2 | What the `node_ptr` null test in `memoize_run` (added by #216) actually excludes | **unverified**: `node_ptr` only reads the DOM node class, and signal / memo objects have a non-null opaque of a different class | read `node_ptr` and the class registration in `crates/nsh-js/src/dom.rs`; log one object of each class |
| 3 | Is "no item memo when there is a single dynamic child" (end of §4.2) a known trade-off | **unverified**: `items.len() < 2` is explicit, no issue found | trace a `<View>` holding one `<If>` whose child is an expression; count Remove / Create |
| 4 | Does an effect writing a signal read by an effect that ran earlier really cost a second run | **inferred**: `write` marks CLEAN readers STALE again | two `effect()`s, the later one writes what the earlier one reads; count the earlier one's log lines |
| 5 | On web (wasm32) only 11 generation bits are kept — can a stale handle be taken for a live one | **unverified**: `GENERATION_MASK` is `0x7ff` on 32-bit | run `unreachable_signals_are_collected` on the web target |
| 6 | Does "a function returning a function" under item_run have a depth limit | **unverified**: `insert_run` has `MAX_DEPTH`, `resolve` has one, `item_run` has none | build a 70-deep chain of `() => () => …` and see whether the stack goes |

Also: `item_memo` is the one symbol the drift check reports missing — it is upstream-only (#216),
not a stale reference. Do not "fix" the map by deleting it.

## 8. Probes

| To learn | Where to instrument | How to read it |
|---|---|---|
| which computations one setter ran | the top of `run_computation` (`kind` + `id`) | log lines around one trace |
| who notified whom | the observer loop in `write` | two adjacent lines (writer id → reader id) |
| an extra pass | the `loop` in `complete_updates` | one line per pass; more than one is shape 3 |
| node remade instead of updated | the `remove_child` / `insert_before` branches of `reconcile` | compare against `SetText` |
| how big the graph is | `Graph::slots()` (used / all) | the slot test in §6 |
| one pass end to end | the `reactive::memo` calls in `insert_items` / `item_run` | per outer-effect run it should equal the number of functions in the array |
| whether the upstream fixes hold | `Create{kind: Input}` / `Remove` / `Delete` in the mount batch | exactly what the #174 / #216 tests assert |

Three that need a build cache (not available here):

```bash
cargo test -p nsh-engine --test engine compiled::a_changing_child_leaves_its_siblings   # #174
cargo test -p nsh-engine --test engine compiled::a_branch_expression_stays_when_a_sibling_changes  # #216
cargo test -p nsh-js --lib reactive::                                              # the scheduler itself
```

---

## Maintenance

A PR that changes this flow **must change this map too**. Then run:

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/reactivity.md .
```

A hit rate below 90% means the map is not trustworthy: fix it before using it.

**Do not add to this map**: call chains (the graph computes them), line numbers (they rot), the full
signature of every function in `reactive.rs` (read the file), every `Op` variant (see
`crates/nsh-core/src/ops.rs`).

---

## 🔴 Where the graph stops (queried 2026-10-21)

This flow spans three crates, and the graph is trustworthy in different amounts in each:

| Symbol | Outbound edges | Good for downstream? |
|---|---|---|
| `write` | 7 (`mark_downstream`, `queue_effect`, `queue_update`, `changed`, `run_updates`, …) | yes [图] |
| `read` | 6 (`track`, `look_upstream`, `update_computation`, `run_updates`, …) | yes [图], but **0 inbound**: the real callers (`signal_call`, the snapshot natives) have no edge |
| `run_updates` | 2 (`complete_updates`, `g`) | yes [图] |
| `insert_value` | 8 (`memoize_run`, `insert_before`, `is_dynamic`, `materialize`, `resolve`, …) | yes [图], but this is **this worktree's** version (no `insert_items`) |
| `memoize_run` | 8, of which `Body.is_null` and `android.collect` are **name collisions** | half: the real callees are `memo`, `array_items`, `new_array` |

Three known traps:

1. **The index is `57b81b5e`, before #174 / #216.** `insert_items`, insert_effect and the renderer's
   item_memo are not in it — not deleted, not yet merged. For the upstream shape, read the blob. Do
   not rebuild this map from the graph:

   ```bash
   git show upstream/main:crates/nsh-js/src/dom.rs
   ```

2. **`insert_items` also hits a test helper of the same name** in
   `crates/nsh-core/src/core/tests.rs`. Same name, other thing.
3. The **free functions** in `crates/nsh-core/src/reactive.rs` have good edges (`write`, `read`,
   `run_top`, `look_upstream`), and so do most `Graph` methods (`track`, `mark_downstream`,
   `clean_node`). What is missing is every edge through a function pointer: `JS_NewCFunctionData`
   hands `insert_run` / `item_run` / `cond_run` / `bind_prop_run` / `each_run` to QuickJS, and the
   graph cannot see that. **"Who calls this C callback" can only be answered by reading code.**

```bash
# whether a symbol's edges can be trusted
codebase-memory-mcp cli --quiet trace_path \
  '{"project":"Users-hxy-Documents-Github-nsh","function_name":"Users-hxy-Documents-Github-nsh.crates.nsh-core.src.reactive.write","direction":"both","depth":1}'
```
