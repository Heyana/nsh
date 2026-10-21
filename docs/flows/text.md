# Flow · text (merge → measure → typeset → glyph)

> The text slice of `docs/flows/frame.md` §2 ③b-5, one level deeper. Read this before touching
> measurement, fonts, line breaking, or anything that "looks fine but scrolls wrong".
> Baseline `5c7060a1` (2026-10-20) · last checked 2026-10-21 · confidence: **confirmed 28 / inferred 2 / gap 6**

Four steps, one owner each:

| step | owner | what it decides |
|---|---|---|
| ① merge | core | one `AttributedText` + one `u64` out of a subtree of spans, raw text and attachments |
| ② measure | taffy + `TextEngine::measure` | a `Size` for a given `max_width` |
| ③ typeset | `TextEngine::layout` | an immutable `TextLayout` for a given box |
| ④ glyph | platform | pixels |

**Cross-platform difference lands in ②–④ (the engine), never in ① (core).** See §4.2.

Facts measured while writing this map (not copied from prose):

| Fact | Value | How |
|---|---|---|
| production `TextEngine` implementations | **7** (macOS, iOS, Windows, Linux, Android, GPUI, Web) `+ ApproxTextEngine` | grep for impl blocks, minus the two test doubles and the `Rc<T>` blanket — [实测] ① |
| `MeasureCache` bound | two generations of 512, so ≤ **1024** live entries | grep for the generation constant — [实测] ② |
| the unit tests of `crates/nsh-core/src/text.rs` pass | 3 passed, 333 filtered out | `cargo test -p nsh-core text:: --lib` — [实测] ③ |

```bash
# ① impl blocks: 10 total = 7 platforms + ApproxTextEngine + Rc<T> blanket + 2 test doubles
grep -rn "impl TextEngine for" crates/
# ② the bound
grep -n "GENERATION" crates/nsh-core/src/text.rs
# ③ the three text tests
cargo test -p nsh-core text:: --lib
```

---

## 1. Trigger points (all of them)

| # | Trigger | Entry symbol | File | Notes |
|---|---|---|---|---|
| 1 | Text content or span structure changes (`Op::CreateText` / `Op::SetText` / child insert or remove) | `Core::mark_text_block` → `dirty::TEXT_BUILD` [读码] | `crates/nsh-core/src/core/mod.rs` | Only the outermost non-inline `Kind::Text` is marked |
| 2 | A text style on an ancestor view (`fontSize`, `color`, `lineHeight`, …) | `Core::mark_inheriting_text` [读码] | same | Walks down, stops at `Kind::Text` / `Kind::Input`, skips `Kind::RawText` |
| 3 | Paragraph props (`numberOfLines`, `ellipsizeMode`, `selectable`) | `Core::mark_text_block` [读码] | same | Goes back through ① |
| 4 | `adjustsFontSizeToFit` / `minimumFontScale` | `Core::queue_text` directly [读码] | same | **No merge**, only re-typeset (`fitted`) |
| 5 | The user's text size changed | `Core::set_font_scale` [读码] | same | Rebuilds the text (size is in the hash); does **not** clear `MeasureCache` |
| 6 | A font is registered (startup / JS `import` of a .ttf / a Rust component's `fonts()`) | `Core::register_font` → `Core::fonts_changed` [读码] | same + `crates/nsh-engine/src/lib.rs` | At startup there are no nodes yet, so `fonts_changed` is a no-op walk |
| 7 | An asynchronous font load finished (**web only**) | `nsh_fonts_loaded` → `EngineControl::fonts_changed` → `Message::FontsChanged` → `Core::fonts_changed` [读码] | `crates/nsh-platform-web/src/lib.rs` | Goes through the message queue, so it lands one `turn` later |
| 8 | A layout pass produced frames to send | `Core::emit_frames` → `Core::queue_text` [读码] | `crates/nsh-core/src/core/mod.rs` | `flush_text` skips unchanged content boxes; usually a no-op |
| 9 | An inline attachment changed size | `Core::check_attachments` → `dirty::TEXT_BUILD` [读码] | same | Re-checked by the `flush_dirty` loop |
| 10 | Input value / placeholder changed | `Core::input_changed` → `Core::measure_input`; `Core::report_content_size` [读码] | same | `Kind::Input` and `Kind::Text` **share** `LeafContext::Text` |

> `Core::flush_text` is **inbound = 1 (and that caller is the `__file__` node), outbound = 0** in the graph [图], yet it calls `measure_text`, `TextEngine::layout` and `Core::place_attachments`. The stage chain below is read from code.

## 2. Stage chain

| Stage | Key symbols | File | In | Out |
|---|---|---|---|---|
| ①a attachments first | `Core::update_attachments` / `Core::attachments_of` [读码] | `crates/nsh-core/src/core/mod.rs` | every non-text, non-inline child of the block | one layout root + size per attachment |
| ①b merge | `Core::collect_text` (recursive) → `AttributedText::push` / `AttributedText::push_attachment` [读码] | same + `crates/nsh-core/src/text.rs` | the block + nested spans + raw text + attachments | one `String` + `Vec<TextSpan>` + `Vec<TextAttachment>` |
| ①c resolve styles | `ResolvedTextStyle::inherit` / `Core::inherited_text_style` [读码] | same | `TextStyle` on the ancestor chain | resolved character style (no more optional fields) |
| ①d key it | `AttributedText::layout_hash` → `TextContext::new` [读码] | same | the merged result | `layout_hash: u64`, computed once per content |
| ①e dirty it | `Core::rebuild_text` [读码] | same | old and new `AttributedText` | `layout_eq` decides whether taffy is dirtied too |
| ②a taffy asks for a content size | the measure closure of `Core::layout_root` → `measure_text` [读码] | same | available width (or `None`) | leaf `Size` |
| ②b two cache levels | `TextContext::cache` → `MeasureCache::get` [读码] | same + `crates/nsh-core/src/text.rs` | `(layout_hash, max_width)` | a hit returns here, a miss falls to ②c |
| ②c platform measure | `TextEngine::prepare` + `TextEngine::measure` [读码] | each platform's `crates/nsh-platform-macos/src/text.rs` and siblings | `&AttributedText` | the engine's own `Size` |
| ②d round up | the `ceil` in `measure_text` [读码] | `crates/nsh-core/src/core/mod.rs` | engine result | only the **rounded-up** value is cached |
| ③ final typeset | `Core::flush_text` → `TextEngine::layout` [读码] | same | content box | `TextLayout` (opaque) |
| ③a report lines | `text_lines` → `EventKind::TextLayout` [读码] | same | `TextEngine::lines` | the `onTextLayout` event (core fills the characters in) |
| ③b place attachments | `Core::place_attachments` → `TextEngine::attachments` [读码] | same | each placeholder's `(x, y)` | `set_frame` for attachment views (cut-off ones move to `-100_000`) |
| ④ hand to the platform | `MountItem::Text { text, layout, content }` → `MountSink::submit` [读码] | `crates/nsh-core/src/mount.rs` | one commit | the platform draws `layout` as-is, it never wraps |
| ⑤ draw | each platform's draw path (CoreText / D2D / Pango / DOM runs) [读码] | each `text.rs` | `TextLayout` | pixels |

> **Measurement happens inside taffy layout (②); the final typeset happens after it, in `flush_text` (③).** The same `Prepared` gets asked for dozens of widths (taffy's min/max-content probes count), so `measure` must be idempotent per width; `layout` runs once (except for `fitted`'s shrink-to-fit).

## 3. Data contracts

**①→② passes an `AttributedText`** (`crates/nsh-core/src/text.rs`), not a DOM-like tree.

| Contract | Must hold | Who depends on it |
|---|---|---|
| Offsets are **UTF-16 code units** | `TextSpan`, `TextLink`, `TextAttachment`, `TextLine` all use one coordinate system | Apple `NSAttributedString`, Android `Spannable`, JS strings, selection copy |
| `spans` are non-overlapping, sorted, and **cover the whole text** | `span_at` binary-searches with `partition_point`; trailing whitespace must belong to a span | `AttributedText::span_at` / `background_at` |
| Neighbouring runs of one style merge | `AttributedText::push` compares `last.style == style` | run count, `layout_hash` stability |
| `transform` (`textTransform`) is applied inside `push` | the key sees **post-transform** characters | case changes cannot corrupt the cache |
| Attachments are U+FFFC placeholders (`ATTACHMENT_CHAR`) | one placeholder per `TextAttachment`, in text order | every engine's run delegate / `ReplacementSpan` |
| `layout_hash` and `layout_eq` **cover the same fields** | §5.1 | cache correctness |
| Only `TextLayout` may cross to the UI thread | it is `Arc<dyn Any + Send + Sync>`; `TextEngine::Prepared` stays on the engine thread | platform drawing |
| `content` is in **view coordinates** (inside padding + border) | `content_box` holds no absolute position | the platform treats it as where the layout goes |
| Cut-off attachments return `None` | the platform moves those views out of sight | `place_attachments` |

**Text node vs. nested span vs. inline attachment** — the section that is easiest to get wrong:

- A `Kind::Text` **block** is the only measured unit and the only taffy leaf. Nested `Kind::Text` (a span) and `Kind::RawText` below it **have no layout of their own**: `collect_text` appends them to the same `AttributedText`, and `ResolvedTextStyle::inherit` stacks the styles.
- Any **other element with a layout** inside the block is an inline attachment: a real layout root (`update_attachments`), represented in the text only by a placeholder whose width/height come from its own content size. It draws itself; the text view gets it `Insert`ed as a child and later `set_frame`d from `TextEngine::attachments`.
- Consequence: **adding a prop to a span changes the block's hash; resizing an attachment changes the block's hash; resizing something inside an attachment changes that attachment's layout first and then forces a rebuild of the block** (`check_attachments`). All three go through the same `dirty::TEXT_BUILD`.
- Conversely `background`, `shadow`, `selectable`, `selection_color` and `links` are **drawing only**: not in the hash, no effect on measurement. `TextBackground` carries `node` precisely so neighbouring spans that look alike still get their own box.

## 4. Branch matrix

### 4.1 Core-side branches

| Dimension | Value | Path | Notes |
|---|---|---|---|
| `no_wrap` | true | in the ① hash; taffy gets `None` (natural width); `flush_text` then raises the content width to the natural one | core owns the width, the engine only measures |
| `max_lines` | 0 / n>0 | 0 = no limit; with n>0, `no_wrap` is forced false | see the `no_wrap` assignment in `rebuild_text` |
| `ellipsize` | tail / head / middle / clip | entirely in the engine; core only passes the enum | `clip` means "no ellipsis glyph" everywhere |
| `adjustsFontSizeToFit` | true | `fitted` bisection (≤7 rounds × 2 measures), **bypassing `MeasureCache`** | §7 |
| `textAlignVertical` | >0 | `flush_text` measures again with `measure_text(…, Some(content.width))` and moves the content down | this one goes through the shared cache |
| `allowFontScaling` / `maxFontSizeMultiplier` | false / finite | `Paragraph::font_multiplier` folds `font_scale` into a multiplier that is multiplied into `font_size` | so the key covers it by construction |
| `writingDirection` / `rtl` | auto / ltr / rtl | core only does `directed_align`; real bidi is the engine's job | **web does no bidi**, §7 |
| Colour-only change | `layout_eq` true | hash kept, taffy *not* dirtied, but `prepared = None` + `laid_out = None` → **a full re-typeset** | platform typesetting state embeds colours |
| Inside a list cell | — | same path, only `layout_cells` instead of `layout_main` | the cache is shared across cells — the main win when scrolling lists |

### 4.2 Where cross-platform differences land

**Not done by core (= engine's business)**: glyph advances, line breaking (which part of UAX #14), bidi reordering, how the ellipsis is cut, how line height is distributed (half-leading vs. min/max line height), baselines, `cap_height` / `x_height`, attachment geometry within a line, `TextLine` metrics.

**Hard-coded in core (= identical everywhere)**: merge and style inheritance, UTF-16 coordinates, the **policy** of `max_lines` / `ellipsize` / `no_wrap`, shrink-to-fit, vertical alignment, round-up, attachment placeholders and their placement, span background boxes (`BackgroundBox::join`), the two cache levels, `layout_eq` / `layout_hash`.

| Platform | Engine | Line breaking | Bidi | Line height |
|---|---|---|---|---|
| macOS / iOS | `CoreTextEngine` | CoreText | native | minimum / maximum line height paragraph settings |
| Windows | `DWriteTextEngine` | DirectWrite | native | own `line_box` (ascent/descent accumulation) |
| Linux | `PangoTextEngine` | Pango (`WrapMode::WordChar`, hyphen insertion off) | native | Pango + `natural_line_heights` for fallback faces |
| Android | `AndroidTextEngine` | `StaticLayout` (Java side) | native | `StaticLayout` |
| GPUI | `GpuiTextEngine` | **own** `unicode-linebreak` | **own** `unicode-bidi` | own |
| Web | `WebTextEngine` | **own** subset of UAX #14 (spaces / CJK / `\n` / long words split per character) | **none** | CSS half-leading |
| Headless / tests | `ApproxTextEngine` | none (0.55em per character) | none | 1.2 × font size |

> **Key consequence**: `TextLayout` is `Arc<dyn Any>` — core can neither see nor change what is inside. So "the two platforms typeset this differently" can never be fixed in core; it lands in one engine, or in ① if the divergence actually comes from the merge. To tell them apart: compare `onTextLayout`'s `start`/`end` between platforms — differ ⇒ engine line breaking; equal but `x`/`width` differ ⇒ engine metrics.

## 5. Invariants

| # | Boundary | Invariant | Who depends on it |
|---|---|---|---|
| 1 | ①d | the fields `layout_hash` covers are **exactly** those `layout_eq` compares: `text`, `align`, `writing_direction`, `rtl`, `max_lines`, `ellipsize`, `no_wrap`, per span `start`/`end` + `font_size`/`font_weight`/`line_height`/`font_family`/`italic`/`letter_spacing`/`font_variant`, per attachment `index`/`width`/`height` | saves a full equality check |
| 2 | ①② | the key is `(layout_hash, max_width)`; `None` is encoded as the bit pattern of `NaN` | §6 |
| 3 | ②c | calling `measure` on one `Prepared` at many widths must give consistent results. **Windows and Pango work by "set the layout's MaxWidth, read it back"** — they rely on re-setting it every time, never on the previous call | core probes min/max-content repeatedly |
| 4 | ②d | what is cached is always the `ceil`ed `Size` | platforms draw on whole points and never clip the last glyph |
| 5 | ③ | while the content box size is unchanged the `TextLayout` is **reused by value** (`Arc` clone), so a platform may memoize on its identity | platform draw caches |
| 6 | ③ | `content` is view-relative, so **moving a text block does not resend `Text`**; a resend happens only when `laid_out` was explicitly cleared (a colour change does this) | `emit_frames` never makes empty items |
| 7 | ③→④ | `Prepared` never crosses a thread; `TextLayout` is `Send + Sync` | core holds the engine in an `Rc<M>`, not an `Arc` — the engine thread owns it |
| 8 | fonts | `fonts_changed` must clear items 1/2/3/4 of §6, dirty taffy and re-queue the text | otherwise stale measurements made with a fallback font live forever |
| 9 | throughout | attachment order = text order = the order `TextEngine::attachments` returns | `place_attachments` pairs them by index |
| 10 | throughout | the cache does **no** equality check, so **a hash collision silently yields a wrong size** | known trade-off, §7 |

## 6. Cache and memoization points

Nine layers sit on this path; four in core (one per `Core`), five in engines/backends.

| # | Cache | Location | Key | Invalidation |
|---|---|---|---|---|
| 1 | shared measurement | `MeasureCache` (`crates/nsh-core/src/text.rs`) | `(layout_hash, max_width)` | replaced wholesale by `Core::fonts_changed`; never shared between processes |
| 2 | single-slot measurement | `TextContext::cache` | the last `max_width` | `rebuild_text` (only when `layout_eq` is false), `fonts_changed` |
| 3 | platform typesetting state | `TextContext::prepared` | the content pointer (no key) | any content inequality (**colours included**), `fonts_changed` |
| 4 | final layout | `TextContext::laid_out` | `(content: Rect, TextLayout)` | same, plus a content change |
| 5 | platform fonts | macOS/iOS `FontCache` (≤64), Windows `faces` + `fonts` (≤256), Linux `fonts` | size bits + weight + family + italic + variant (Linux uses `pango::Font`) | **each engine's own `register_font`**, not `Core::fonts_changed` |
| 6 | web advances | `WebTextEngine`'s `widths` | `(CSS font string, exact substring)` | `clear_caches`; cleared wholesale past 20 000 entries |
| 7 | web metrics | `WebTextEngine`'s `metrics` | CSS font string | `clear_caches` |
| 8 | web JS metrics | `metricsCache` (`crates/nsh-platform-web/web/nsh.js`) | CSS font string | cleared when all fonts finished loading |
| 9 | Blink shaping | the `PlainTextPainter` behind the canvas 2D context | font + string (inside Blink) | **no invalidation API**: a new context every `MEASURE_ROUNDS` (5000) measurements |

### 6.1 What the `MeasureCache` key must cover

Already covered by `layout_hash` (**must** be there, all in `AttributedText::layout_hash`):

| Input | Why it must be in the key |
|---|---|
| characters after transform | a different character is a different size |
| each run's `start`/`end` | same characters, different run split, different metrics (mixed scripts) |
| `font_size`, `font_weight`, `italic`, `font_family`, `font_variant` | they select the glyphs |
| `line_height` (**already resolved to points**) | decides line height; `lineHeight: 1.5` and `lineHeight: 21` hash to the same value, deliberately |
| `letter_spacing` (**already resolved to points**) | the advance of every atom |
| `align`, `writing_direction`, `rtl` | engines exist where these change breaking or line width |
| `max_lines`, `ellipsize`, `no_wrap` | "measure the first n lines only" changes the height |
| each attachment's `index`/`width`/`height` | the placeholder takes part in breaking and line height |
| `max_width` (the second half of the key) | the same content can be asked at any width |

Deliberately **not** covered (covering them would be wrong):

| Input | Why it must stay out |
|---|---|
| `color`, `decoration*`, `background`, `shadow` | drawing only; in the key, a colour change would re-measure the screen |
| `selectable`, `selection_color`, `links` | interaction and drawing |
| `ResolvedTextStyle.line_height_multiple`, `TextStyle.letter_spacing_em`, `TextStyle.transform` | already resolved to points/characters in ①; hashing them again is redundant |
| `font_scale` | already multiplied into `font_size`, so it is covered indirectly |

**Not** covered, and therefore dependent on invalidation — the point of this table:

| Input | Today |
|---|---|
| the font data itself (same family name, different glyphs; or a family that was missing and now exists) | not in the key. `Core::fonts_changed` compensates by *clearing* caches, never by *changing* the key |
| device pixel ratio, Android's `density` | not in the key, and not a variable (fixed when the engine is built); web's `measureText` works in CSS px, so DPR never enters a measurement |
| the platform font caches (layer 5) | **`Core::fonts_changed` does not clear them.** Desktop relies on each `register_font` doing it (registration is synchronous, so that suffices); Linux's `PangoTextEngine.line_heights` (font description string → ascent/descent) is cleared by nobody — §7 |

> 🔴 `docs/flows/frame.md` §6, the row saying "web shaping cache key = as above + device pixel ratio", **is wrong**: the web keys are only `(CSS font string, substring)`, with no DPR [读码]. This section supersedes it.

### 6.2 What must be re-measured after `fonts_changed`

What `Core::fonts_changed` does (all of it necessary; drop one line and a stale measurement survives):

| Action | Why |
|---|---|
| `measure_cache = MeasureCache::default()` | the layer-1 key cannot see fonts, so the only fix is to drop everything |
| per `Kind::Text` / `Kind::Input` (non-inline, with a layout): `prepared = None` | the platform typesetting state embeds resolved fonts (CTFont / DWrite format / Pango font / CSS font string) |
| same: `cache = None` | the single-slot measurement |
| same: `laid_out = None` | otherwise a size that happens to be unchanged `continue`s and the platform never gets the new layout |
| same: `layout.mark_dirty` + `needs_layout = true` | line heights and widths changed, taffy must recompute |
| `Kind::Text`: `queue_text`; `Kind::Input`: `mark(dirty::INPUT)` | the former re-typesets and resends `MountItem::Text`; the latter makes the platform resolve the input's font again |

**Nothing else needs re-measuring**: canvas `measureText` (`canvas::measure_line` never touches `MeasureCache`, so core holds no stale value; web's own layers 6/7/8 are handled by `clear_caches`), attachment layout roots (ordinary nodes), `links` / `background` (not in the key).

**Ordering (web)**: `nsh_fonts_loaded` clears web layers 6/7 and JS layer 8 synchronously, then posts `Message::FontsChanged`, which clears core layers 1–4 one `turn` later. In between, an engine-level width is fresh while core still holds a cached `Size` — harmless (the cache is only read by `measure_text`), but it is why the two halves exist. Note the asymmetry: **native never calls `EngineControl::fonts_changed`**, only web does, because only web loads fonts asynchronously. Anyone adding async fonts to a native platform must extend this chain; today it stops at web.

## 7. Unverified list

| # | Question | Today | How to verify |
|---|---|---|---|
| 1 | Linux `natural_metrics`'s font-description cache (`PangoTextEngine.line_heights`) is never cleared by `register_font` | **reading the code suggests a real gap**: the string is the *requested* family, which resolved to a fallback before registration, so a cached value stays wrong forever | register a family that shadows a system one with different glyphs; compare `natural_metrics` before and after |
| 2 | Should `letter_spacing` follow `font_scale`? | **two places disagree**: the `scaled` used by `Core::rebuild_text` multiplies only `font_size` / `line_height`; the closure inside `fitted` multiplies `letter_spacing` too | test `letterSpacing` in em with `fontScale 2` and read the span's `letter_spacing` |
| 3 | Web does no bidi reordering at all | read from code: `WebTextEngine` uses `rtl` only to resolve `Auto` alignment and lays atoms in logical order; no `unicode-bidi` in its dependencies. What that costs in practice is unmeasured | compare `onTextLayout` `start`/`end` for "Latin + Arabic" on web vs. macOS |
| 4 | Web clamps the width of `no_wrap` text asked with `Some(max_width)` | Windows / Pango skip that clamp when `no_wrap`; web does not. The only caller that asks this way reads `.height`, so it is invisible today | unit-test `WebTextEngine`: `no_wrap` + `Some(w)`, read `measure().width` |
| 5 | What a hash collision actually costs | `MeasureCache` explicitly skips the equality check, so a collision is a silent wrong size | sample `layout_hash` distribution, or add a debug-only full comparison in `insert` |
| 6 | The real cost of `fitted` bypassing `MeasureCache` | up to 7×2 prepare+measure per content change, and `fitted` clones the whole `AttributedText` each time | count calls inside `fitted`; run a list with `adjustsFontSizeToFit` |

## 8. Observable probes

| Want to know | Where to instrument | How to read it |
|---|---|---|
| cache hit rate | the two return branches of `MeasureCache::get` | two atomic counters divided by the `get` count |
| measurements per commit | the entry of `measure_text` | read together with the `Core::commit_round` count |
| how many `prepare`s | the call sites of `TextEngine::prepare` in each engine | one prepare is one full shaping pass |
| whether a layout was reused | the three branches on `laid_out` inside `flush_text` | count each (hit / moved only / re-typeset) |
| the blast radius of a font change | the `blocks` length inside `Core::fonts_changed` | that number is how many text blocks get fully re-typeset |
| commit cost | the core section of `nsh::perf` (typesetting is already inside it) | `nsh trace` or `Engine::set_perf_monitor` |
| whether the web shaping cache grows | `MEASURE_ROUNDS` rebuilds | count inside `measure_text`; 5000 is one tick |

---

## Maintenance

A PR that changes this flow **must change this map too**. Then run:

```bash
python ~/.workbuddy/skills/flow-map/check.py docs/flows/text.md .
```

Hit rate < 90% = this map cannot be trusted; fix it before using it.

**Do not add to this map**: line numbers (they rot), the full field list of `AttributedText` (see `crates/nsh-core/src/text.rs`), each engine's line-breaking details (see its own `text.rs`), every field of `ResolvedTextStyle`.

---

## 🔴 Graph coverage boundary (2026-10-21, all numbers from the graph itself) [图]

| Symbol | inbound | outbound | Note |
|---|---|---|---|
| `Core::flush_text` | 1 (`__file__`) | **0** | calls four functions in reality |
| `Core::rebuild_text` | 1 (`__file__`) | **0** | same |
| `Core::fonts_changed` | 2 | **0** | same |
| `AttributedText::layout_hash` | 3 | 3, **all wrong** (resolved to `nsh-js`'s `CallHost.finish` / `iterator.new`) | name too generic; do not trust it |

**Conclusion: the text chain in `nsh-core` must be read from code.** The only place this map and the graph overlap is the trigger table in §1 — and even that was read from code.