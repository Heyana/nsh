# Flow · Build and hot reload (save → screen)

> Read this before touching the bundler, the dev server, HMR or `.nsh`.
> Baseline `57b81b5e` (2026-10-21) · last checked 2026-10-21 · confidence: 89 `[code]` / 3 `[graph]` / 12 `[probe]` tags (counted with `grep -o`) · gaps 8 (§7)

Sources are tagged per row: `[graph]` = `codebase-memory-mcp`; `[code]` = read from source; `[probe]` = ran the command in the row. **`[graph]` is only trustworthy where a row also says `[code]`** — see *Graph coverage* at the end.

---

## 1. Triggers (all of them)

| # | Trigger | Entry symbol | File | Source |
|---|---|---|---|---|
| 1 | A save of `.ts` / `.tsx` / `.js` / `.jsx` / `.json` outside `.nsh` and `node_modules` | `DevSession::watch_until` (`is_source`, `DEBOUNCE = 30ms`) | `crates/nsh-cli/src/dev.rs` | `[code]` |
| 2 | A save of the app's Rust code (`.rs`, `Cargo.toml`, `assets/*` of a crate `nsh.json` `"rust"` lists, or of a local path dependency) | `RustApp::is_rust` → `Restart` → `main::run`'s loop | `crates/nsh-cli/src/rust_app.rs`, `crates/nsh-cli/src/main.rs` | `[code]` |
| 3 | Session start's first build | `DevSession::start` → `DevSession::rebuild` | `crates/nsh-cli/src/dev.rs` | `[code]` |
| 4 | `nsh build` | `build::build` → `compile` / `compile_split` | `crates/nsh-cli/src/build.rs` | `[code]+[probe]` |
| 5 | A host connecting and receiving the latest bundle | `DevServer::publish` → `send_event` (server) → `DevEvent::Bundle` (host) | `crates/nsh-dev/src/server.rs`, `crates/nsh-cli/src/dev.rs` | `[code]` |
| 6 | A hot patch reaching a running host | `DevEvent::Patch` → `dev::handle` → `EngineControl::hot_update` | `nsh-platform-macos/src/dev.rs` (and each platform's `dev.rs`) | `[code]` |
| 7 | Cmd+R / the web page's runtime-change reload | `reload_latest`, `dev.html`'s `nsh.version` check | `nsh-platform-macos/src/dev.rs`, `crates/nsh-cli/src/web/dev.html` | `[code]` |
| 8 | `nsh check` — type check only, **no build** | `check::check` (spawns `tsc -p`) | `crates/nsh-cli/src/check.rs` | `[code]` |

> 🔴 `nsh dev` and `nsh run` share one session implementation: `nsh run` starts `DevSession` first, then launches the host with `--dev-server <ws url>` (`crates/nsh-cli/src/run.rs`). `nsh dev` serves hosts that connect by themselves. **Same pipeline, different start.**

## 2. Stage chain

| Stage | Process / thread | Key symbols | File | In → Out | Source |
|---|---|---|---|---|---|
| ① watch | CLI main thread creates the watcher; `notify` calls back on its own thread | `notify::recommended_watcher`, mpsc | `nsh-cli/src/dev.rs` | fs events → channel | `[code]` |
| ② filter + coalesce | CLI main thread | `is_source`, `DEBOUNCE` | same | events → one `rebuild` per 30 ms burst | `[code]+[probe]` |
| ③ bundle (dev) | CLI main thread; `bundle_hot` builds a **current-thread** tokio runtime and `block_on`s it | `build::compile_hot` → `nsh_bundler::bundle_hot` | `nsh-cli/src/build.rs`, `nsh-bundler/src/bundle.rs` | sources → `HotBundle` (one CJS chunk per module) | `[code]` |
| ③a per module | rolldown plugin hook | `compile::compile_native` | `nsh-bundler/src/compile.rs` | TSX → JS | `[code]` |
| ③a-1 TypeScript strip | " | oxc `Transformer` (`only_remove_type_imports`) | same | types gone | `[code]` |
| ③a-2 hot transform | " | `hot::transform` — **before** JSX goes | `nsh-bundler/src/hot.rs` | `component` / `context` / `keep` wrappers | `[code]` |
| ③a-3 JSX | " | `jsx::transform` (`PRIMITIVES`, `HELPERS`) | `nsh-bundler/src/jsx.rs` | JSX → `element` / `bindProp` / `insert` calls | `[code]` |
| ③a-4 elide + codegen | " | `elide_unused_imports`, oxc `Codegen` | `nsh-bundler/src/compile.rs` | JS + source map | `[code]` |
| ③b class check | " | `classes::check` (`nsh_core::tailwind::parse`) | `nsh-bundler/src/classes.rs` | unknown classes → `nsh::build` warnings only | `[code]` |
| ③c assets + natives | " | `NshPlugin::load`, `NshPlugin::resolve_id` | `nsh-bundler/src/plugin.rs` | images/fonts → modules; native specifiers → `natives` | `[code]` |
| ③d rolldown | rolldown's own tasks inside the current-thread runtime | `Bundler::generate` | `nsh-bundler/src/bundle.rs` | graph → chunks (`Cjs`, `preserveModules`, `treeshake: false`) | `[code]` |
| ④ script | CLI main thread | `HotBundle::script` | same | chunks → one ES module: `import "nsh:hot"`, native namespaces, `__nsh_hot.define(...)` × N, `__nsh_hot.start(entry, {…})` | `[code]` |
| ⑤ bytecode | CLI main thread | `nsh_js::compile_bytecode_with` | `nsh-cli/src/build.rs` | **whole** script → QuickJS bytecode | `[code]` |
| ⑥ patch | CLI main thread | `HotBundle::patch(previous)` | `nsh-bundler/src/bundle.rs` | two bundles → `Unchanged` / `Update{code,map}` / `Reload` | `[graph]`+`[code]` |
| ⑦ publish | CLI main thread → server runtime | `DevServer::publish` (version += 1) | `nsh-dev/src/server.rs` | patch event, then bundle event, `latest = Bundle` | `[code]` |
| ⑧ transport | `nsh-dev-server` worker threads (2), one task per socket | `serve_client`, `send_event` | same | events → WS text frame + 1–2 binary frames | `[code]` |
| ⑨ host receive | host's `nsh-dev-client` thread | `client::session` → `on_event` | `nsh-dev/src/client.rs` | frames → `DevEvent` | `[code]` |
| ⑩ hop to main | host main thread | `DispatchQueue::main().exec_async` → `dev::handle` | `nsh-platform-macos/src/dev.rs` | `DevEvent` → `control.hot_update` | `[code]` |
| ⑪ apply | engine thread (web: the page's single thread) | `Message::Hot` → `state.hot` → `Script::hot_update` → `Host::eval_script` → `__nsh_hot.update` | `nsh-engine/src/lib.rs`, `nsh-engine/src/script.rs`, `nsh-js/src/hot.rs` | patch script → re-run modules, bump component versions | `[code]` |
| ⑫ commit | same engine thread, same `turn` | `State::commit` → `MountSink::submit` | `nsh-engine/src/lib.rs` | dirty state → `MountBatch` | `[code]` |
| ⑬ apply on screen | platform main/UI thread | macOS `Sink::send` → `Surface::drain` → `Mounter::apply` | `nsh-platform-macos/src/surface.rs` | batch → native views (see `docs/flows/frame.md`) | `[code]` |

### 2.1 Who runs where

| Process | Threads that matter | Source |
|---|---|---|
| `nsh` (CLI) | main (watch loop, bundling, bytecode, patch); `notify`'s callback thread; dev-server tokio workers ×2 (`nsh-dev-server`) | `nsh-cli/src/dev.rs`, `nsh-dev/src/server.rs` |
| host process (macOS/Windows/Linux/GPUI/iOS/Android) | `nsh-dev-client` (socket); main/UI thread (events, `hot_update` hand-off, batch apply); engine thread (`Driver::turn`) | `nsh-dev/src/client.rs`, `nsh-platform-*/src/dev.rs` |
| browser (web host) | one thread: `dev.html` → `nsh.js` → wasm; `nsh_hot_update` applies and `pump()`s **synchronously** | `nsh-cli/src/web/dev.html`, `nsh-platform-web/src/lib.rs` |

### 2.2 One save, in order

1. `notify` sees the write → channel → the CLI's 200 ms poll → `is_source` (skips `Access` events, `.nsh`, `node_modules`) → drain 30 ms → `rebuild`.
2. `rebuild` sends `Building` to every host (`ServerMessage::Building` → "Building…" pill).
3. `compile_hot` re-bundles **everything** (no incremental cache) and compiles the **whole** hot script to bytecode.
4. `HotBundle::patch(previous)` diffs module text; the patch is a JS script, never bytecode.
5. `publish` bumps `version`, sends `Patch{version, base}` then `Bundle{version}`.
6. Each host: client thread → main thread → `running == (session, base)`? apply : drop.
7. Engine `turn`: `state.hot` → `eval_script` → `__nsh_hot.update(defs)` → re-run changed modules (topologically, dependencies first, importers when a module cannot take its own update) → bump each stale component's version signal.
8. Same `turn`: timers, tasks, `commit` → `MountBatch` → platform.

## 3. Data contracts

| Contract | Must hold | Who depends on it | Source |
|---|---|---|---|
| `HotModule.id` | equal to the chunk file name (= path relative to the app root, `App.js`); it is the `require` key **and** the patch key | `__nsh_hot.define/update`, `require`, `resolve` | `[code]` |
| Patch shape | `__nsh_hot.update({"App.js": function (module, exports, require) {…}, …});`, evaluating to a boolean | `HotBundle::patch`, `Script::hot_update` | `[code]` |
| `Patch { version, base }` | `base == version - 1`; a host applies it only if it runs exactly `base` in the same `session` | `dev::handle`, `dev.html` | `[code]` |
| Frame order | `patch` / `bundle` text frame, then payload, then (if `source_map`) the map — one frame each, always in that order | `client::session`'s `Pending`, `dev.html`'s `pending` | `[code]` |
| `BundleFormat` | bytecode only when the host's `bytecode_version == BYTECODE_VERSION`; empty string (web) always means source | `DevServer::start`, `serve_client` | `[code]` |
| `ui_sets` | a host that does not serve every set the bundle imports gets `CannotRun` and no bundle | `Host::missing`, `send_event` | `[code]` |
| `session` / `version` | `session` differs per server process; `version` is monotonic within one session | host's `running: Option<(session, version)>` | `[code]` |
| `HotPatch::Unchanged` | sent as the literal script `"true"`, so hosts can clear a stale error without reloading | `DevSession::rebuild` | `[code]` |
| `capabilities`, `app_id` | travel **only** on `Bundle`; never on `Patch` | `Bundle::with_app_id`, `with_capabilities_json` | `[code]` |

### 3.1 What HMR keeps, and what it cannot

| Kept across a patch | Lost |
|---|---|
| the JS context and every module that did not re-run | a re-run component's own local state (its `signal`s are rebuilt) |
| `exports` object identity of re-run modules (old object forwards to the new one through getters) | module-level mutable state not wrapped by `keep` |
| top-level `const x = signal(…)` / `createNavigator(…)`, while the initializer's text is unchanged (`keep`, `KEPT = ["signal", "createNavigator"]`) | a kept value whose initializer text changed (a new one is made) |
| `context()` identity (first one under a name wins, takes the new default) | — |
| the mounted tree and native views: `mount(App)` with the same proxy keeps what it mounted | other `mount` calls of a re-run module are undone from the first that differs |
| event handlers bound to nodes that were not re-rendered | bindings a re-run module made at its top level (`undo_on_rerun`, e.g. shortcuts) |

Source: `[code]` (`nsh-js/src/hot.rs`, `nsh-bundler/src/hot.rs`), asserted by `crates/nsh-engine/tests/engine/hot.rs`.

### 3.2 Build artifacts and `.nsh/types`

| Artifact | Written by | Content | Read by | Source |
|---|---|---|---|---|
| `bundle.js` | `build::build` | the entry chunk: an ES module importing native modules (`nsh_core::js::ENTRY_CHUNK`) | hosts, `/bundle.js` on the dev server, packaged sites | `[code]` |
| `bundle.js.map` | same | source map (hidden, not referenced from the code) | symbolicator, host error overlay | `[code]` |
| `bundle.nshc` | same | QuickJS bytecode of `bundle.js` | hosts/packages, **only** if `BYTECODE_VERSION` matches | `[code]` |
| `Screen.js` / `Screen.js.map` / `Screen.nshc` | `compile_split` | one chunk per `import()` target | runtime `__nsh_import`, web site | `[code]` |
| `app.json` | `build::build` | `{"id": …}` — where the app keeps its data | host runners | `[code]+[probe]` |
| `capabilities.json` | same, when `nsh.json` declares capabilities | JSON | host | `[code]` |
| `assets/<path>` | same | the files `asset:/<path>` names | host | `[code]` |
| `*.nshpkg` | `nsh_package::build` (`nsh publish`) | magic `NSHP` + JSON header + files + Ed25519 signature over everything before it; **always carries the source**, bytecode is an extra | host, only after verification | `[code]` |
| `.nsh/types/**`, `.nsh/tsconfig.json` | `init::write_types` | `nsh_js::type_definitions(ui_sets)` + `nsh_bundler::type_definitions`, and the base tsconfig | the editor and `nsh check` (`tsc -p`) | `[code]` |

**How they relate — one input, three outputs.** `crate::ui_sets()` (`nsh_core::js::NATIVE_MODULES` + `nsh_core::runtime::registries()` + the app's `"rust"` crates as described) feeds:

1. the bundler, which treats those specifiers as external and records them as `Bundle::natives` (`NativeModules`);
2. `.nsh/types`, which declares exactly those modules;
3. the host handshake, which advertises the same set specifiers.

The bundler **never reads** `tsconfig.json` or `.nsh/tsconfig.json` (`TsConfig::Auto(false)`), and `nsh check` is a separate process, so a stale or broken `.nsh/types` can neither change a bundle nor block `nsh dev`.

`[probe]` `nsh build examples/counter/App.tsx --out /tmp/nsh-build-probe` → `3.2 KB js, 9.8 KB bytecode, 19ms`; `bundle.js` 3255 B, `bundle.js.map` 5699 B, `bundle.nshc` 9993 B, `app.json` 28 B (`{"id":"com.example.counter"}`), and `find examples/counter/.nsh/types -name '*.d.ts' | wc -l` → 22.

`[probe]` `nsh dev examples/counter/App.tsx` → `4.1 KB js, 15.5 KB bytecode, 9ms`; two saves → `rebuilt … (13ms)` and `(25ms)`. The dev script is bigger than the release bundle: it keeps every module separate and disables tree shaking.

`[probe]` the same session, then `touch` three files in turn: `.nsh/types/nsh/probe.d.ts` → nothing, `notes.txt` → nothing, `helper.ts` → one `rebuilt`. Confirms invariant 5 (a `.d.ts` under `.nsh` would otherwise match the `ts` extension).

`[probe]` `cargo test -p nsh-engine --test engine hot::` → 2 passed: `edits_replace_modules_and_keep_unrelated_state` (component edit, plain module edit, entry edit, a throwing component, a newly imported native module → `Reload`) and `entry_edits_keep_navigation_and_state` (a pushed screen and a kept signal survive an entry edit; a changed initializer starts afresh). `cargo test -p nsh-bundler --lib` → 25 passed, including `bundles_ignore_the_apps_tsconfig`.

## 4. Branch matrix

| Dimension | Value | Path | Note | Source |
|---|---|---|---|---|
| Session mode | `nsh dev` / `nsh run` | `compile_hot` + `HotPatch` | HMR | `[code]` |
| Session mode | `--release` | `build::compile(entry, true)`, `patch = None` | every save is a full reload (new engine) | `[code]` |
| Patch outcome | `Unchanged` | script `"true"` | clears the error overlay, no re-render | `[code]` |
| Patch outcome | `Update` | `Script::hot_update` → `eval_script` | in place | `[code]` |
| Patch outcome | `Reload` | no patch; the bundle that follows is loaded from scratch | see the five `Reload` causes below | `[code]` |
| `Reload` cause | entry module name changed | `self.entry != previous.entry` | rename = new app | `[code]` |
| `Reload` cause | a native module newly imported | `!self.natives.all(in previous)` | asserted by `edits_replace_modules_and_keep_unrelated_state` | `[code]` |
| `Reload` cause | a changed chunk is rolldown's own runtime (`\0` module ids) | `embedded` | never replaced in place | `[code]` |
| `Reload` cause | `capabilities` changed | `dev.rs` drops the patch (`previous.capabilities == hot.capabilities`) | app restarts under new capabilities | `[code]` |
| `Reload` cause | first build (`version == 1`) | `publish` sends no `Patch` | nothing to diff | `[code]` |
| `update()` result | `true` | applied; `log::info!("hot update applied in {}µs")` | — | `[code]` |
| `update()` result | `false` | `MountSink::hot_update_failed` → host reloads the latest bundle | `affected()` returned `None`: a changed module that accepts no self-update, has **no importer** and is not the entry | `[code]` |
| `update()` result | throws | `Script::report("hot update failed")` → error overlay, **no reload** | the modules that re-ran stay; the next patch can fix the rest | `[code]` |
| Host kind | native | `hot_update_failed` → `set_hot_update_failed` → `reload()` | per-platform `dev.rs` | `[code]` |
| Host kind | web | `nsh_hot_failed()` → `dev.html` sets `running = null` | the following bundle loads from scratch | `[code]` |
| Payload | host bytecode version ≠ server | `BundleFormat::Source` + a warning | web advertises `""` | `[code]` |
| App shape | plain TSX | bundle + types + hosts | — | `[code]` |
| App shape | TSX + `"rust"` crates | `describe` → `.nsh/rust` → hosts linked with the crates | Rust change = restart | `[code]` |
| App shape | Rust app (`"root"`) | no bundle at all (`build::rust_root`); `rebuild` returns early | only assets, `app.json`, the web page | `[code]` |

## 5. Invariants

| # | Boundary | Invariant | Who depends on it | Source |
|---|---|---|---|---|
| 1 | ③ | every save rebuilds the **whole** app (bundle + bytecode); HMR only decides which modules re-run | nothing incremental exists to get wrong | `[code]+[probe]` |
| 2 | ⑥→⑩ | a patch applies only to the exact `base` of the same `session` | `dev::handle`, `dev.html` | `[code]` |
| 3 | ⑥ | a patch is always JS **source**, never bytecode — bytecode hosts still get HMR | `Script::hot_update` | `[code]` |
| 4 | ⑪ | `false` ⇒ the host reloads; a throw ⇒ overlay, no reload. Never conflate them | `Script::hot_update` | `[code]` |
| 5 | ① | nothing under `.nsh/` or `node_modules/` triggers a rebuild — and `.d.ts` **does** end in `.ts`, so only the path check saves you | no rebuild loop from `write_types` | `[code]+[probe]` |
| 6 | ⑦→⑩ | a host already at `version` ignores the bundle of that version that follows the patch | avoids applying a patch and then reloading it | `[code]` |
| 7 | ③→⑤ | `bundle.js` and `bundle.nshc` come from the same `code` string; bytecode is adopted only when `BYTECODE_VERSION` matches | host, `.nshpkg` | `[code]` |
| 8 | ③ | bundling ignores `tsconfig.json` (paths, aliases come from `nsh.json`) | `.nsh/types` can never break a build | `[code]+[probe]` (`bundles_ignore_the_apps_tsconfig`) |
| 9 | ⑪ | a module "accepts" its own update only if it registered ≥ 1 component and **all** its enumerable exports are component proxies; otherwise the change climbs to its importers, up to the entry | `hot::accepts`, `hot::affected` | `[code]` |
| 10 | ⑪ | `keep` values survive only while their initializer's *text* hash is unchanged | top-level signals, navigators | `[code]` |
| 11 | Rust | a component's implementation is compiled into the host binary; no JS patch can replace it | the whole `Restart` loop | `[code]` |
| 12 | ⑦ | `version` is monotonic per `session`; `session` differs per server process | hosts tell builds of two runs apart | `[code]` |

## 6. Cache points

| Cache | Where | Key must cover | Invalidation | Source |
|---|---|---|---|---|
| last good `HotBundle` | `DevSession.hot` (`RefCell`) | — | replaced on every good build; **kept** on a failed one (a failed build keeps serving the previous) | `[code]` |
| `.nsh/rust/description.json` | generated workspace | the describe program's binary stamp | Rust code change → new stamp | `[code]` |
| `.nsh/types` writes | `init::write_types` | file content (identical content is not rewritten, so editors do not reload) | content change; stale files are deleted | `[code]` |
| dev server `latest` + broadcast(16) | `Shared` | — | a lagging client re-reads `latest`, possibly skipping intermediate patches | `[code]` |
| hot runtime state (`Hot`) | inside the JS context | — | dies with the VM: a reload is a cold start | `[code]` |
| Cargo build | app's `target/` or `CARGO_TARGET_DIR` | Cargo's own | source change | `[code]` |
| bundling / oxc | none | — | every save re-bundles from scratch | `[code]+[probe]` |

> 🔴 Two different "caches" are easy to confuse: `DevSession.hot` decides what a **patch** contains, and the VM's `Hot` decides what a **patch does**. Clearing one without the other yields a patch that either over- or under-applies.

## 7. Unverified

| # | Question | Now | How to verify |
|---|---|---|---|
| 1 | **Deleting** a module (or a top-level statement) while a session runs | `HotBundle::patch` only compares modules present in the **new** bundle, so a deletion may yield `Unchanged` while the host marks the new version as running | delete a module only the entry imports, save, watch for `Reload` vs `Unchanged` |
| 2 | A `nsh.json` `tailwind` / `theme` / `appearance` change | the entry re-runs and re-executes `_$setup(…)`, but whether live nodes re-take theme tokens is unchecked | change `nsh.json` theme with the app on screen |
| 3 | Time split within one rebuild (rolldown vs oxc vs bytecode vs patch) | only `Built::elapsed` and `HotBundle::elapsed_ms` exist; neither is broken down or printed | instrument `compile_hot` |
| 4 | `app_id` change | travels only on `Bundle`; a host that reached that version through a patch never sees it | edit `nsh.json` `"id"` and inspect `log::info!("loading build …")` |
| 5 | Changing nsh's own Rust crates (a checkout) while `nsh dev` serves a TSX-only app | `RustApp::watched` only covers the app's crates and their local path dependencies; the prebuilt host may be stale | edit `nsh-core`, run `nsh dev`, check the host binary's mtime |
| 6 | Rust-app (`"root"`) web host: `RUST && !running` runs the root once; a changed runtime relies on the `nsh.version` reload | code reads right, never run | `nsh run --target web` on `examples/gallery-rs`, save Rust, watch the page reload |
| 7 | Broadcast lag (> 16 events) in a burst | expected: hosts stay on an old version until the next whole bundle | throttle the bundler artificially, save repeatedly |
| 8 | `nsh dev --release` full reload per save | expected from `patch = None` + `AppView::load` starting a new engine | run it with a host attached |

## 8. Probes

| Want to know | Where | How to read |
|---|---|---|
| whether the rebuild fired, and how long it took | `DevSession::published` | `✓ built … (9ms)` / `✓ rebuilt … (13ms), 2 devices` |
| bundle/bytecode sizes | `Built::summary` | `(4.1 KB js, 15.5 KB bytecode, 9ms)` |
| whether a patch applied in place | `Script::hot_update` | `hot update applied in Nµs` |
| whether a patch failed | same | `hot update failed: …` (target `js`), or the host's `reloading after an update that could not be applied in place` |
| which modules re-ran | `__nsh_hot.update` defs + `hot::affected` | log the keys of `defs` and the returned `order` |
| which build a host runs | `DevUi.running` / `dev.html`'s `running` | `(session, version)`; host logs `loading build N` / `updating to build N` |
| hosts that cannot run the app | `Host::missing` | `CannotRun` + the CLI's `… its host has no …` |
| unknown Tailwind classes | `NshPlugin::transform` | `nsh::build` warning with line/column |
| bytecode handshake mismatch | `serve_client` | `…: bytecode … != …, sending JS source` |
| watch filter behaviour | `is_source` | `nsh dev` prints nothing for a save under `.nsh/` (verified) |

---

## Maintenance

Any PR that changes this flow **must** update this page. Then run:

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/build.md .
```

A hit rate below 90 % means this page cannot be trusted: fix it before using it.

Known checker artifact: `check.py` may report `hot_update_failed` as missing, because `hot_update` appears earlier in this page and shadows it inside the script's single alternation. The symbol exists (`MountSink::hot_update_failed`, `crates/nsh-engine/src/lib.rs`).

**Do not add here**: full field lists of `HotPatch` / `ServerMessage` (read the files), line numbers (they rot), or the per-platform error-overlay UI.

---

## Graph coverage (measured 2026-10-21)

This page is mostly `[code]` because the graph is unreliable exactly where this flow lives:

| Query | Result |
|---|---|
| `HotBundle::patch`, `direction: both` | 14 callees, most of them wrong (`Capabilities::all`, `Bounds::max`, `xtask::collect`) — the class-level group prefix leaks unrelated methods |
| `DevSession::rebuild`, `direction: both` | 66 callees, including `nsh-platform-gpui` canvas and image code — unusable for a chain |
| `DevSession::rebuild` → `HotBundle::patch` | hop **1**, consistent with the code; this is the one edge worth trusting |
| `Script::hot_update` | **not found** (`nsh-engine/src/script.rs`'s `imp` module is not indexed under the name you would guess) |
| `init::write_types` inbound | `main::run` and `dispatch`, consistent with the code |

Rule of thumb: use the graph to find a definition or to confirm one edge; read the source for every chain.
