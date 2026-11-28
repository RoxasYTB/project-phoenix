# Patches to GPUI (`gpui-pre` 0.3.12)

`vendor/gpui-pre` is the crates.io `gpui-pre` 0.3.12 source, wired in through
`[patch.crates-io]` in the workspace `Cargo.toml` (and excluded from the workspace
members). Examples, benches, tests, docs and the lockfile were removed from the copy,
with their `[[example]]`/`[[bench]]`/`[[test]]` tables, to keep it small. The library
source is untouched except for the patches listed here. Each one is marked
`PHOENIX PATCH` in the code.

To upgrade GPUI: copy the new release into `vendor/gpui-pre`, re-apply the patches below,
and bump the `=0.3.12` pins in `crates/phoenix-app/Cargo.toml`.

## 1. Subpixel layout (`src/taffy.rs`, `impl ToTaffy<f32> for AbsoluteLength`)

**Problem.** Upstream rounds every authored length (padding, margin, gap, width, height)
to whole device pixels *before* layout. At a fractional scale such as 125 % this makes
lengths drift: a 38px channel row (1 + 36 + 1) is laid out as 1 + 45 + 1 = 47 device px
instead of 47.5. The official client (Chromium) keeps layout fractional and snaps only
the painted edges. So our lists drifted by 0.5 dp per row: about 3 px after 6 rows in the
channel list, with the same drift in the guild rail (measured with `scripts/compare`
against the live client at DPR 1.25).

**Patch.** `AbsoluteLength::to_taffy` returns the exact device-pixel value
(`logical * scale_factor`) instead of `round_half_toward_zero(logical * scale_factor)`.
The rest of upstream's snapping is kept:
- `layout_bounds` still rounds each element's outer edges in absolute device pixels after
  layout. That is one snapping step at "paint", and coincident edges still close.
- Border widths still go through `round_stroke_to_device_pixel`, so a 1px border stays
  1 dp and is never blurred.
- Measured text/leaf sizes are still ceiled to device pixels.

At integer scale factors (1x, 2x) authored integer lengths are already whole device
pixels, so the output there is unchanged.

## 2. Fractional measured sizes (`src/element.rs` `Drawable::layout_as_root`, `src/window.rs`, `src/taffy.rs`)

**Problem.** `list` measures each item as its own layout root and stacks the measured
heights. The measured size was the snapped one, and the far edge of a root at origin 0
rounds half toward zero (47.5 becomes 47), so with patch 1 alone the channel list still
lost 0.5 dp per row.

**Patch.** `layout_as_root` returns the raw taffy size (new
`Window::layout_size_unsnapped` / `TaffyLayoutEngine::layout_size_unsnapped`). Items are
then placed at fractional offsets, and `layout_bounds` snaps their edges in absolute
device pixels at paint, so they close with no drift.

## Effect (live compare, DPR 1.25, same screen)

| region | upstream | patched |
|---|---|---|
| guild rail | 14.2 % | 2.6 % (patch 1) |
| member list | 16.0 % | 12.3 % (patch 1) |
| messages | 6.8 % | 5.4 % (patch 1) |
| overall | 8.1 % | 6.7 % (patch 1) |

## How the patch is wired (shared build cache)

`Cargo.toml` patches `gpui-pre` with a **git rev of this repository**, not a path.
Cargo gives a path dependency a different package id in every checkout or worktree, which
rebuilt GPUI and everything above it (gpui-component, the platform crates…) once per worktree.
A git rev has one id everywhere, so all checkouts share one compiled GPUI.

After changing `vendor/gpui-pre`: commit and push it, then set `rev` in `Cargo.toml`
to that commit and run `cargo update -p gpui-pre`.

## 3. Paint only the children that were prepainted (`src/elements/div.rs`, `src/element.rs`)

**Problem.** The real app panicked intermittently with `must call prepaint before paint`
in `paint<Stateful<Div>>`. `Div::prepaint` and `Div::paint` each recompute the style
and skip the children when `display == None`. Hover and group-hover styles are part of
that style, and the hover state can differ between the two calls (the group hitbox of
the current frame is only known after prepaint). A div that is `hidden()` and becomes
`flex()` on group hover (the voice channel row icons in `ui/sidebar.rs`) skipped its
children in prepaint and then painted them: the `vc-chat` stateful child panicked.

**Patch.** `Div` remembers which children it prepainted (`prepainted_children`) and
paint follows that list instead of reading `display` again. The panic messages of
`Drawable::prepaint`/`paint` now name the element type, its id, its source location and
the phase it was in.

## Page zoom (`src/window.rs`, `src/interactive.rs`, `src/platform.rs`)

**Problem.** Discord's zoom (Ctrl+= / Ctrl+- / Ctrl+0, Appearance > Zoom level) is
Electron `webFrame.setZoomFactor`, i.e. Chromium page zoom: every CSS px, border, icon,
hit area, scrollbar and popout position scales together, and the viewport shrinks in
CSS px. Scaling lengths by hand in the app (`z(v)`) left every unwrapped `px()` at 100 %.

**Patch.** `Window::set_page_zoom(f32)` / `Window::page_zoom()`. The window keeps the raw
platform scale factor and content size; the effective `scale_factor` becomes
`platform * zoom` and `viewport_size` becomes `content / zoom`. `dispatch_event` divides
input positions (and pixel scroll deltas) by the zoom, and the values going back to the
platform (window menu position, client inset, input region, IME bounds, character index
lookups) are multiplied by it. Search `PHOENIX PATCH (page zoom)`.

## 4. Chromium-style ellipsis (`src/elements/text.rs`, `src/text_system/line_wrapper.rs`)

**Problem.** Phoenix cut truncated labels one character before the client at the same
width ("Happy 03…" against "Happy 03/1…" in the member list). Two causes: upstream trims
spaces and ASCII punctuation before the "…", and it decides the cut on a sum of
per-character advances, which ignores kerning and overestimates the shaped width.

**Reference.** Chromium 152 (the client's engine), probed in headless Chrome with a
monospace font so every cut is readable:
- `white-space: nowrap; text-overflow: ellipsis` keeps the longest prefix whose width
  plus the ellipsis fits, spaces and punctuation included ("Happy …", "ab, …", "ab.…").
- `-webkit-line-clamp` ends the last line at its own soft wrap, drops the collapsible
  space, then appends "…" ("bbbb…", "cccc, dddd.…"). It cuts inside a word only when the
  word plus the ellipsis does not fit. Punctuation is kept.

**Patch.** `LineWrapper::truncate_line_end_shaped` cuts one shaped line at the last whole
grapheme whose shaped advance (`LineLayout::x_for_index`) plus the affix fits. `Text`
uses it for single-line end truncation (it already shaped the text to test the fit).
The per-character fallback and `truncate_wrapped_line` no longer trim punctuation, and
the last clamped line stops at its last word break when one fits. Search
`PHOENIX PATCH (ellipsis)`.

The commit is kept on the `gpui-ellipsis` branch of origin so the pinned `rev` stays
reachable.

## 5. Glyph offsets (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `layout_line_no_separators`)

**Problem.** Combining marks painted one letter to the right of their base: the bot name
"ღ🌸~͓̽ǤŁa̠̠ĐØS~🌸ღ" in Aperture Sciences showed the U+0320 macrons under "Đ" instead of "a".
`cosmic_text::LayoutGlyph::x`/`y` are the pen position only; the shaper's GPOS offsets are in
`x_offset`/`y_offset` (em units) and cosmic-text adds them in `LayoutGlyph::physical`. GPUI
copied `x`/`y` and dropped the offsets, so a zero-advance mark sat at the pen position after
its base.

**Patch.** `ShapedGlyph::position` is `(x + x_offset·size, y − y_offset·size)`, like
`physical`. Advances, and so widths and `x_for_index`, are unchanged. Search
`PHOENIX PATCH (glyph offsets)`.

## 6. Cluster fallback (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `compute_run_spans`)

**Problem.** The same name was 1.8px wider than in the client and got an ellipsis. GPUI picks
the font of a grapheme from its first character, so "~" + U+033D U+0353 and "a" + U+0320 U+0320
stayed in gg sans (which has "~" and "a" but no marks). cosmic-text then reshaped the whole
cluster in its own fallback, the system "Noto Sans" Regular (9.15 and 8.98px). Chromium gives
the cluster to the first font of the stack that covers every character (CDP
`CSS.getPlatformFontsForNode`: Noto Sans Medium, the web font, 8.15 and 8.18px).

**Reference.** Live client, 2026-11-24, `Range.getBoundingClientRect` on a probe span with
`font-family: var(--font-primary)` 16px/500: total 140.01px; prefix widths per grapheme in
`docs/parity/text.md`.

**Patch.** `pick_grapheme_slot` checks every non-default-ignorable character of a multi-char
grapheme: primary font if it covers all of them, else the first font of the fallback chain
that does, else the old first-character choice (cosmic-text's fallback takes over). Single
characters keep upstream's `pick_covering_slot`. Search `PHOENIX PATCH (cluster fallback)`.
Unit tests: `cluster_goes_to_the_first_font_covering_every_mark`,
`ignorables_do_not_need_a_glyph`, `uncovered_cluster_keeps_the_first_character_choice`.

## Wiring of `gpui-pre-wgpu`

`vendor/gpui-pre-wgpu` is the crates.io 0.3.12 source (bench table and `benches/` removed),
patched like `gpui-pre` through a git rev of this repository in `[patch.crates-io]`. Both
crates point at the same rev, kept on the `gpui-marks` branch of origin (it is
`gpui-ellipsis` + `vendor/gpui-pre-wgpu`). Only the two `[[package]]` entries of
`Cargo.lock` change when the rev moves: edit them by hand, because `cargo update -p` also
re-resolves unrelated crates (itertools, windows-sys).

## 7. Text box width (`src/elements/text.rs`, `ceil_to_layout_unit`)

**Problem.** After patches 5 and 6 the bot name had the client's advances but still got an
ellipsis: the "✓ APP" badge beside it was 46px wide against 44.7px in the client (compare
finish/ellipsis-1280x720, DPR 1.5). Upstream rounds every measured text box up to a whole
logical pixel, so "APP" in gg sans 12px/600 (23.16px, CDP `getBoundingClientRect`) took 24px.

**Patch.** The measured width is rounded up to 1/64 px (Chromium's LayoutUnit) instead. It still
never rounds down, so a text given its own measured width does not re-wrap. Unit test
`text_width_rounds_up_to_a_layout_unit`. Search `PHOENIX PATCH (text width)`.

## 8. Underline geometry (`src/text_system/line.rs` `paint_line`, `src/text_system.rs`, `src/text_system/line_layout.rs`)

**Problem.** Underlines (hovered names, links) followed the height of each piece of text:
"霜氷冷 | 冰雪 ❄" showed one line per glyph group at different heights. `paint_line` placed
the underline from `LineLayout::ascent`/`descent`, the maximum over every font the line
used, so a CJK or emoji fallback moved it, and labels made of several shaped pieces (emoji
labels, gradient names painted grapheme by grapheme) each drew their own line. GPUI also
kept the caller's 1px thickness, thinner than the client's.

**Reference.** Blink draws one `text-decoration: underline` per decorating box from its
primary font. Headless Chrome 158 with the client's gg sans and Journal (from the local
Discord cache), 12/14/16/20px, line-height 11/8, scale 1, 1.1, 1.2, 1.25, 1.3, 1.4, 1.5,
1.75 and 2: 72 probes, all matched by (device pixels):
- ascent A and descent D of the primary font, each rounded;
- half leading `floor((line_height - A - D) / 2)`;
- thickness `max(1, floor(font_size / 10))`;
- gap below the baseline `max(1, ceil(font_size / 20))`;
- top of the line `round(row_top + half_leading + A + gap)`.
CJK text in a gg sans span puts the line exactly where Latin text does (probe
`霜氷冷 | 冰雪 ❄` = `catapulteur2daronnes`: row 17 at scale 1, 21-22 at 1.25).

**Patch.** `ShapedLine` and `WrappedLine` keep `primary_font`, the font the first run asked
for (set by `shape_line` / `shape_text`), and `paint_line` reads its ascent and descent;
`LineLayout::paint` (bare layouts) falls back to the line maximum. `LineLayout` itself is
untouched: gpui-base builds it with struct literals. `underline_geometry()` is the
formula above; `paint_line` uses it for the position of every non-wavy underline and its
thickness, per wrapped row. Wavy underlines keep their style. Search
`PHOENIX PATCH (underline geometry)`. Unit test `underline_geometry_matches_chromium`; the
three underline handler tests compute their expectation with it.

Phoenix's `NameText` (gradient and letter-spaced names, painted one grapheme at a time)
draws its single underline with the same function and the metrics of a space in the
primary font.

## 9. Primary-font baseline (`src/text_system/line.rs` `paint_line`)

**Problem.** On a name with emoji the underline sat 1 dp too high against the glyphs. `paint_line`
put the baseline at `(line_height − ascent − descent) / 2 + ascent` with the line's maximum ascent
and descent over every font it used, so a fallback font (Noto Color Emoji, an Egyptian hieroglyph
face…) moved every glyph of the line, and `NameText`, which shapes one grapheme at a time, gave each
pictograph its own baseline.

**Reference.** Chromium keeps the baseline of the strut: the primary font's ascent and descent plus
the half leading of `line-height`; fallback fonts never move it while `line-height` is not `normal`.
Live client 2026-11-25, probe divs `font-family: var(--font-primary)` 16px/20px with a zero-size
inline-block at the baseline: "Nyxie" and "🌸Nyxie🌸𓂃" (gg sans + Noto Color Emoji + Noto Sans
Egyptian Hieroglyphs) both 15.2 px below the top. Headless Chrome 158, scale 1.25: 15.2 for gg sans
16/20 with or without emoji, 18.4 for Pricedown 20/24/600 with or without "🌸˖𓂃❀𝐍𝐲𝐱𝐢𝐞❀𓂃˖🌸".

**Patch.** The baseline (and the strikethrough) use the metrics of `primary_font`, the font the
first run asked for, like the underline of patch 8. Bare `LineLayout::paint` keeps the line maximum.
For a line that only uses its primary font the metrics are the same values (cosmic-text's
`max_ascent` is the same swash ascent × size), so plain text does not move. Careful: the cosmic
backend's `FontMetrics::descent` is negative (swash's positive descent, negated) while
`LineLayout::descent` is positive, so the patch takes `.abs()` of both, like `underline_geometry`.
The first version (rev `4c7d4f5a`) missed it and lowered every line by the font's descent
(about 0.3 em); fixed in `0c04d8e7`. Search
`PHOENIX PATCH (primary baseline)`.

## 10. Emoji presentation selectors (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `pick_grapheme_slot`)

**Problem.** "❄️" (U+2744 U+FE0F) was drawn in DejaVu Sans: GPUI picked the font from the base
character and U+FE0F is default-ignorable.

**Reference.** Live client and headless Chrome 158, 2026-11-25, `CSS.getPlatformFontsForNode` on
probe spans in `var(--font-primary)` 16px: ❄️ ❤️ ☃️ ✔️ ▶️ ☺️ ↔️ ™️ ©️ #️⃣ → Noto Color Emoji (19.97 px),
even when gg sans has the base (↔, ©, ™); a️ stays in gg sans; 🔊︎ → FreeSerif 10.09, ⭐︎ → FreeSans
10.73, ❄︎ ❤︎ ⚡︎ → DejaVu Sans, 🌸︎ → Symbola. A bare emoji-presentation character goes to the colour
font unless a font of the CSS stack has it (font-family FreeSerif: 🔊 → FreeSerif).

**Patch.** For a grapheme whose base can be an emoji (non-ASCII, or `#`, `*`, a digit): with U+FE0F,
the first font of the chain that is a known emoji font and covers it; with U+FE0E, the first
non-emoji font (primary, sticky current, then the chain); otherwise the usual choice, except that a
character whose first covering font in the chain is an emoji font goes there instead of the sticky
current fallback (so "𝓢🔊" still draws 🔊 in colour now that the text faces keep their emoji code
points). Phoenix places a second copy of Noto Color Emoji with every non-ASCII code point
("Phx Noto Color Emoji Selector") at the end of the chain for the text-default bases. Search
`PHOENIX PATCH (emoji presentation)`. Tests `vs16_goes_to_the_emoji_font_before_text_fonts`,
`vs15_goes_to_a_text_font_and_bare_emoji_skips_the_sticky_one`.

## 11. Synthetic bold (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `layout_line_no_separators`, `render_glyph_image`)

**Problem.** In 600+ text, a symbol from a fallback face (★ ❄ ☲ 𝔻 from DejaVu Sans, 𝐍 from
FreeSans, ⓒ from IPAGothic) was drawn as thin as in regular text.

**Reference.** Headless Chrome 158 and the live client, 2026-11-25, `var(--font-primary)` 20px:
`getPlatformFontsForNode` names the regular face at every weight (DejaVuSans, FreeSans, IPAGothic,
Symbola, not DejaVuSans-Bold) and the widths do not change (★❄☲𝔻𝐍 84.06 px at 400, 550, 600 and
700), but from 600 the glyphs are drawn heavier (ink +27 to +84 % on the headless probes); 550 is unchanged. With
`font-family: 'DejaVu Sans'` named in CSS, 550+ uses the real DejaVuSans-Bold. That is Skia's
FreeType path: `FT_Outline_Embolden` with strength ppem / 24 (device pixels), advances unchanged.

**Patch.** When a run asks for 600 or more and a glyph comes from a face lighter than 600 (and not a
colour emoji font), the glyph gets a twin `FontId` of the same face marked `synthetic_bold`;
rasterizing it calls swash's `embolden(ppem / 48)` (swash's port of `FT_Outline_EmboldenXY` does not
halve the strength as FreeType does). Shaping and advances are untouched. Search
`PHOENIX PATCH (synthetic bold)`. Test `synthetic_bold_follows_chromium`.

## 12. Rich lines (`src/text_system/rich.rs`, `src/text_system.rs`, `src/text_system/line.rs`)

**Problem.** The composer's Markdown preview needs what the client's Slate leaves get from CSS
(chunk 358931 and 44141, see `docs/parity/composer.md`): inline code in `--font-code` at 85 % inside
a 1px bordered, rounded box, spoilers with `padding: 0 .2em` and rounded corners, subtext at 13px.
A `TextRun` has one font size per line, and `HighlightStyle` cannot change the size, draw a border
or add space. `TextRun`, `ShapedRun` and `LineLayout` are built with struct literals in gpui-base,
so new fields on them would break that crate.

**Patch.** A parallel description instead of new run fields:
- `RichText { sizes, boxes, hidden }`, `TextBox { range, background, border_color, border_width,
  corner_radius, padding, clone_edges, open_start, open_end }` (builders `TextBox::new(r).background(..)`…).
  `RichText::slice` cuts it for one wrapped row: a box cut by the row edge loses its padding,
  border and corners on that side (`box-decoration-break: slice`) unless `clone_edges` is set
  (`clone`, the spoiler's `spoilerMarkdownContent_`).
- `WindowTextSystem::shape_line_rich(text, font_size, runs, &rich, force_width)`: each range with
  its own size is shaped as a separate segment through `layout_line` (so each segment stays in the
  frame cache) and the segments are joined into one `LineLayout`. A box inserts
  `padding + border` before its first glyph and after its last one; a hidden range drops its glyphs
  and its width. `x_for_index`, `index_for_x` and `closest_index_for_x` then work unchanged, so the
  caret, the selection and IME bounds follow the drawn text. `layout_line_rich` returns the same
  width for wrapping. An empty `RichText` is the old `shape_line`.
- `ShapedLine` keeps a private `rich` (`run_sizes` and the box geometry). `paint_line` rasterizes
  each run at its own size; the baseline stays the strut of the primary font at the line's size,
  like an inline box in Chromium. `ShapedLine::paint_background` paints each box as one quad
  (background, border on the closed sides, `corner_radius` on the closed corners). Its height is
  the content area of the box's font at its size (ascent + descent) plus the border, around the
  baseline, like an inline element's background in CSS.

Tests in `rich.rs`: `sizes_lay_out_each_segment_at_its_own_size`,
`boxes_add_their_padding_and_border_to_the_advance`, `adjacent_boxes_do_not_overlap`,
`slicing_opens_cut_edges_unless_cloned`, `hidden_text_takes_no_space`,
`empty_rich_text_is_the_plain_shape`, `boxes_paint_one_rounded_bordered_quad`.

## 13. gpui-base: rich textarea (`vendor/gpui-base`)

`vendor/gpui-base` is the crates.io `gpui-base` 0.10.0 source (the engine behind gpui-component's
`Input`/`Textarea`), without tests, benches, dev-dependencies and lockfile, patched through the same
git rev as `gpui-pre` (`[patch.crates-io]`). It is not a member of `vendor/Cargo.toml` (excluded), so
the vendored test run stays on gpui-pre and gpui-pre-wgpu. gpui-component does not re-export the new
types: Phoenix depends on `gpui-base` directly.

- `RichSpan { range, font_family, font_size, text_box, hidden }` and
  `LineStyle { range, bar: LineBar { color, width, radius }, indent, end_inset, font_family, font_size }`
  (`input/base/rich_text.rs`), set with `TextareaState::set_rich_decorations(spans, lines, cx)` and
  read back with `rich_decorations().spans()/.lines()`. Ranges follow edits like the text decorations
  and are dropped when the text is replaced.
- Layout (`base/element.rs` `layout_lines`, `layout_token_lines`): a line with rich content is shaped
  with `shape_line_rich` per wrapped row, its runs get the spans' and the line's font family, the line
  font size replaces the field's. A line style's `indent` becomes `LineLayout::lead`, added to every
  row by `line_indent` (caret, selection, hit testing and paint all go through it). The bar is painted
  under the row(s) in `paint_background`; consecutive barred lines square their shared corners (the
  client's `slateBlockquoteContainer_:has(+…)` rules). Token lines now also paint backgrounds.
- Wrapping (`display_map/text_wrapper.rs`): the wrapper keeps a copy of the rich decorations (with the
  weights and styles of the text decorations, so bold text wraps at its real width) and measures a
  rich line with `layout_line_rich` at `wrap_width − indent − end_inset`. `set_rich` rewraps only the
  rows whose own view (`RichDecorations::line_view`) changed, so typing does not rewrap the field.
  Lines with inline tokens keep the token wrapper (plain widths).

Tests: the app tests below drive it (gpui-base's own tests are not in the vendored test workspace).

## 14. LayoutUnit ellipsis (`src/taffy.rs` `snap_measured_size_to_device_pixels`, `src/elements/text.rs`, `src/text_system/line_wrapper.rs` `truncate_line_end_shaped`)

**Problem.** Texts next to an emoji or a measured sibling lost their last glyph to the
ellipsis: member group "💻 ─ ☲☲ Responsable du Dé…" for the client's "Dév…", composer
placeholder "#〢🍯・hone…" for "honey…" (sweep audit-live-channel, DPR 1.25 and 1.5). Box
widths and glyph positions matched within 1 device px. The bridge to taffy rounded every
measured leaf up to a whole device pixel, so the count "\u{a0}— 2" (24.958px) took 25.333px at
DPR 1.5 and the name box lost 0.375px. "Dév" + "…" fits the client's 191.042px box with
0.08px to spare (harfbuzz on the cached gg sans 500 14px: "Dév" ends at 180.811, "…" 10.15).

**Reference.** Chromium sizes a text box in LayoutUnits, 1/64 of a device pixel at
zoom-for-DSF: the dump widths are whole multiples of 1/96 css px at DPR 1.5 (191.042 × 96 =
18340, 122.51 × 96 = 11761). `LineTruncator` keeps the glyphs whose end fits in the box
minus the ellipsis' `SnappedWidth()` (rounded up to a LayoutUnit), and the first box of a
line keeps at least one character.

**Patch.** Measured widths round up to 1/64 dp (heights keep the whole device pixel), the
text box width of patch 7 rounds up to 1/64 dp too, and `truncate_line_end_shaped` takes the
scale factor, rounds the ellipsis width up to 1/64 dp and keeps the first grapheme when
nothing fits. An emoji sequence is one grapheme. Unit tests
`measured_width_rounds_up_to_a_device_layout_unit`, `text_width_rounds_up_to_a_layout_unit`,
`shaped_end_cut_keeps_the_last_glyph_that_fits_like_chromium`,
`shaped_end_truncation_counts_an_emoji_as_one_glyph`; app tests in `ui::emoji_text`.

**Check, 2026-11-26.** `honeypot_placeholder_cuts_after_honey_like_the_client` replays the
whole composer placeholder of Aperture #honeypot (« Envoyer un message dans #〢🍯・honeypot »,
gg sans 400 16px, box 290.667px at DPR 1.5) with its harfbuzz ends. « honey » ends at
279.314px and the ellipsis is 11.232px, rounded up to 1079/96, so the cut keeps « honey » with 0.11px
to spare. The test is `end ≤ box − SnappedWidth(…)`, like Chromium's `LineTruncator`: an end
exactly on the budget is kept, and 0.001px less cuts it. Phoenix renders « honey… » with
patch 14 and the composer's `mr(12)`, so the cut itself needed no change. The « hone… » seen
before came from the box, not from the cut.

Rev: `7ebb03d2` on `gpui-gradient-anim2` (patches 4 to 17). Before: `22d0f05c` on `gpui-gradient-anim` (patch 17 without the benchmark hooks), `d00bee73` on `gpui-gradient` (patches 4 to 16), `776d5b2c` on `gpui-rewrap` (patches 4 to 15), `fc9bf4e7` on `gpui-ellipsis2` (patches 4 to 14), `5e340a80` on `gpui-richtext` (patches 4 to 13), `0c04d8e7` on `gpui-baseline2` (patches 4 to 11), `4c7d4f5a` on `gpui-baseline` (patch 9 with the descent sign bug), `0ce4a85b` on `gpui-underline`, `265d2211` on `gpui-marks`.

## 15. Rewrap slack (`src/elements/text.rs` `needs_rewrap`)

**Problem.** After patch 14 a chip sized to its own text could break its last glyph onto a
second line: server tags « GLA » + « D » and « Zz » + « z » over the status line in the
member list (`real_shots` #honeypot, DPR 1.5). The text's prepaint wraps again when the
painted width differs from the last measured one by more than 0.5px. `layout_bounds`
rounds both edges to device pixels, so a box paints up to one device pixel narrower than
its layout width (0.667px at 1.5x, 0.8px at 1.25x). With widths in whole device pixels this
loss was hidden; with LayoutUnit widths it went over the 0.5px threshold.

**Reference.** Chromium wraps at the layout width and only snaps the painted rect, so the
snapping never re-wraps a line.

**Patch.** The rewrap needs a change of more than `max(0.5px, 1 device px)`, and wraps at the
painted width plus one device pixel. Unit test `snapping_to_device_pixels_does_not_rewrap`.
Search `PHOENIX PATCH (rewrap slack)`.

## 16. Text gradient (`src/text_system/gradient.rs`, `src/scene.rs`, `src/window.rs` `paint_glyph`, `vendor/gpui-pre-wgpu/src/shaders_gradient.wgsl`, `wgpu_renderer.rs`)

**Problem.** Role gradients and display name styles were drawn one solid colour per
letter (`NameText` sampled the gradient at each grapheme's centre). The client paints a
real `background-clip: text` gradient, so a wide letter goes from one colour to the next
inside the glyph, and the vertical part of the diagonal name gradient was lost.

**Reference.** Client CSS (local cache, 2026-11-26):
- roles, `447505.1c681faad626d654.css`: `.twoColorGradient_HG2xvK{linear-gradient(to right, c1, c2, c1)}`,
  `.threeColorGradient_HG2xvK{linear-gradient(to right, c1, c2, c3, c1)}`,
  `.usernameGradient_HG2xvK{background-clip:text; background-size:100px auto}` (repeats every 100px);
- display name styles, `163466.71f5984e9faac7fc.css`: `.gradient_or4Hj-{linear-gradient(to bottom right, stops); background-size:100% auto}`,
  `.prism_or4Hj-{linear-gradient(90deg, prism stops); background-repeat:repeat; background-size:var(--…-prism-cycle) auto}`;
  stops from `abd83513bd56377e.js` `UserNameWithEffects`: GRADIENT/GLOW `c_i (10 + 80 i/(n-1))%`,
  PRISM `c_(i mod n) (100 i/n)%` for i = 0..n, cycle `max(<name length>ch, 38px)`.

**Patch.** `TextGradient { angle, stops, tile, repeat_x }` is a CSS linear gradient in
logical pixels (`TextGradient::to_corner` gives the angle of `to bottom right` for a tile).
`Window::with_text_gradient(Some(&g), |window| …)` paints every monochrome glyph drawn inside
it with the gradient: the glyph stays a `MonochromeSprite`, its `pad` holds `1 + index` into
`Scene::glyph_gradients` (`GlyphGradient`: device-pixel origin, gradient step and offset,
repeat period, up to 8 stops, opacity), and `Scene::replay` copies the gradient of a reused
sprite. Subpixel AA is off for those glyphs, like Chromium's grayscale text with a clipped
background. The scene enums are unchanged, so the macOS / Windows renderers still build (they
draw the first stop). The wgpu renderer writes the gradient sprites (sprite + gradient) to
their own instance range only when the scene has gradients, and splits a mono batch into runs:
plain runs keep the mono pipeline, gradient runs use `vs_gradient_sprite` / `fs_gradient_sprite`
(`shaders_gradient.wgsl`), which compute `t = dot(p − origin, step) + offset` per pixel (x
wrapped by the period), interpolate the stops in sRGB like CSS and apply the same contrast
and gamma correction as the mono shader. A scene without gradients takes exactly the old path.
WebGL has no gradient pipeline and falls back to the solid mono pipeline. Search
`PHOENIX PATCH (text gradient)`.

Tests: `text_system::gradient::tests` (horizontal, repeated, `to bottom right` corners,
device pixels, stop clamping, out-of-order stops), wgpu `gradient_shader_is_valid_wgsl`,
`gradient_sprite_record_matches_the_shader_layout`, `headless_renderer_fills_gradient_glyphs_per_pixel`
(plain sprite stays solid, gradient sprite ramps per pixel). App: `fonts::name_paint::tests`,
`fonts::tests::role_gradients_go_back_to_the_first_colour_like_the_client_classes`,
`prism_stops_cycle_back_to_the_first_colour_over_the_name_length`.

## 17. Gradient scroll and text glow (`src/text_system/gradient.rs`, `src/text_system/glyph_blur.rs`, `src/scene.rs`, `src/window.rs` `present` / `paint_glyph`, `src/platform.rs` `AtlasKey`, wgpu `write_gradient_sprites`)

**Problem.** Patch 16 painted the gradients still. The client scrolls them: role gradients
while the message or member row is hovered, the PRISM display name style on hover or
selection, and a hovered role name gets a blurred copy behind it. Animating through a
`request_animation_frame` would notify the view and lay out and paint the whole chat
on every frame.

**Reference.** Client CSS (local cache, 2026-11-26):
- `447505.1c681faad626d654.css`: `.full-motion.app-focused:not(.hardware-acceleration-disabled) .animateGradient_HG2xvK{animation:gradientUsernameAnimation_HG2xvK 1.5s linear infinite}`,
  `@keyframes gradientUsernameAnimation_HG2xvK{0%{background-position:0}to{background-position:100px}}`;
  `.usernameGlow_HG2xvK{filter:blur(4px);opacity:0;z-index:-1}`, `.usernameGlowActive_HG2xvK{opacity:.7}`,
  `.convenienceGlowGradient_HG2xvK:after{background:inherit;background-clip:text;content:attr(data-text);filter:blur(4px);opacity:0;transition:opacity .1s ease-in-out;z-index:-1}`,
  `.convenienceGlowGradientActive_HG2xvK:after{opacity:.7}`;
- `163466.71f5984e9faac7fc.css`: `.animated_or4Hj- .prism_or4Hj-{animation:prism-scroll_or4Hj- 2s linear;animation-fill-mode:forwards}`,
  `.animated_or4Hj-.loop_or4Hj-.loop_or4Hj->*{animation-iteration-count:infinite}`,
  `@keyframes prism-scroll_or4Hj-{0%{background-position-x:0}to{background-position-x:calc(var(--custom-display-name-styles-prism-cycle)*-1)}}`.

**Patch.**
- `TextGradient::scroll(Some(GradientScroll { speed, start, duration }))`: `speed` logical px per
  second of `background-position-x`, from `start` (an `Instant`), for `duration` (`None` = forever;
  a finite one stops on its last frame, `fill-mode: forwards`). `Window::with_text_gradient`
  turns it into a `GlyphGradientScroll` (device px per second, start on `gradient_clock`) stored
  next to the gradient (`Scene::glyph_gradient_scrolls`, same index, copied by `replay`).
- `Window::present` stamps `Scene::gradient_time` with `gradient_clock(now)` before drawing;
  `write_gradient_sprites` reads `Scene::glyph_gradient_now`, which moves the gradient tile right
  by `speed × elapsed` wrapped by the repeat period. The shader and the instance layout are
  unchanged.
- While a scroll is still running at that time (`Scene::scrolls_gradients`), `present` sets
  `needs_present` and schedules a frame. The next platform frame takes the present-only branch of
  the frame callback: no view is notified, nothing is laid out or painted, the last scene is drawn
  again at the new time. A hover change or any other redraw rebuilds the scene as usual.
- `Window::with_text_glow(Some((blur, opacity)), |window| …)` paints the monochrome glyphs drawn
  inside it as a gaussian blur of their mask (`filter: blur(blur)`: standard deviation `blur`
  logical px, cut at 3σ, the sprite padded by 3σ) at `opacity` times the element opacity, with the
  current text gradient if any. The blurred mask is computed once per glyph (`blur_mask`,
  separable, CPU) and kept in the atlas under `AtlasKey::BlurredGlyph(params, σ bits)`.
- `Window::set_gradient_elapsed` (test-support) sets the clock of the current frame to a time after
  its first scroll, for frame-by-frame captures (`real_shots` `PHX_GRADIENT_AT`).
- `HeadlessAppContext::simulate_frame_request` (test-support) delivers one platform frame request,
  so a benchmark drives the real draw or present-only branch; `Window::scrolls_gradients` says
  whether the current frame keeps re-presenting (`real_shots` `PHX_PRESENT_CPU`).

Tests: `text_system::gradient::tests::scroll_moves_the_tile_right_and_wraps_by_the_period`,
`a_finite_scroll_stops_on_its_last_frame_like_fill_mode_forwards`,
`the_clock_orders_instants_on_both_sides_of_its_epoch`; `text_system::glyph_blur::tests`
(padding, gaussian spread and coverage, edge at half coverage, no blur); wgpu
`headless_renderer_scrolls_gradient_glyphs_at_the_scene_time`. App:
`fonts::name_paint::tests::role_gradient_scrolls_100px_to_the_right_every_1_5s_like_the_client_keyframes`,
`prism_scrolls_its_cycle_to_the_left_in_2s_once_or_forever`,
`reduced_motion_window_focus_and_still_names_do_not_scroll`, `only_moving_role_gradients_glow`.

**CPU, 2026-11-26.** `real_shots` `PHX_PRESENT_CPU=10` on the real account (Aperture #général,
1536×840 at DPR 1.5, Clem's « Coordinateur de Sécurité » #9e6bff → #9fc1ff name hovered, so the
gradient scrolls and glows), under `phoenix-nice`, frame requests every 16 ms, two runs:

| phase | CPU (one core) | frame requests in 10 s |
|---|---|---|
| hovered name scrolling, present only (patch 17) | 58.9 % / 56.6 % | 257 / 271 |
| hovered name scrolling, full redraw every frame (what `request_animation_frame` would do) | 72.1 % / 73.3 % | 173 / 166 |
| idle, mouse gone, nothing scrolls | 57.6 % / 59.5 % | 264 / 250 |

The ~58 % floor is the harness itself (the headless test dispatcher, gateway and media work of
the real account) and is the same with nothing on screen. A scrolling name stays within the
±2 points noise of idle, while redrawing the view each frame costs 14 to 16 more points at only
~17 frames per second (about 8 ms of CPU per frame). Frames in
`/mnt/windata/project-phoenix/compare/gradient-anim` (`PHX_GRADIENT_AT=0,375,750,1125`, private).

Not done: the blur is applied to the glyph coverage, then filled with the gradient, where Chromium
blurs the painted colours. With a 100px gradient period and a 4px blur the colour difference is
under one step of the ramp. (The glow's 0.1s opacity transition came with patch 18.)

## 18. Text stroke, text shadow and keyframe animations (`src/text_system/glyph_blur.rs` `dilate_mask`, `src/text_system/text_animation.rs`, `src/scene.rs` `glyph_animations` / `glyph_sprite_now`, `src/window.rs` `with_text_stroke` / `with_text_animation` / `paint_glyph` / `paint_svg`, wgpu `write_gradient_sprites`)

Branch `gpui-name-effects` (on top of `gpui-gradient-anim2`), rev `42aa62d2`.

**Problem.** The NEON, TOON, POP and GUMMY display name styles draw a text stroke under the fill,
a blurred text shadow, a second copy of the name behind or above it, and 4 s keyframe
animations of `transform`, `background-position` and `color`. GPUI had none of these for text, and
a `request_animation_frame` per frame would redraw the whole view (patch 17 measured +14 to 16
points of one core). The role dot in « À côté des noms » mode is an SVG circle filled with a
reflected gradient that SMIL scrolls, and the glow of patch 17 had no 0.1s fade.

**Reference.** Client CSS `163466.71f5984e9faac7fc.css` and JS `abd83513bd56377e.js`
(`UserNameWithEffects`), `web.b63d846b329d6470.js` module 501803 `$0` (palette), module 545442
(role dot SVG); details and values in `docs/parity/chat.md`.

**Patch.**
- `Window::with_text_stroke(Some(width), |window| …)`: monochrome glyphs drawn inside are the glyph
  mask dilated by `width / 2` (CSS `-webkit-text-stroke-width` with `paint-order: stroke fill`: the
  fill covers the inner half of the stroke). `dilate_mask` takes the max of the coverage over a disc
  of that radius in device pixels, each source pixel weighted by how much of it lies inside the
  disc (`clamp(r + 0.5 − d, 0, 1)`), padded by `ceil(r) + 1`. With `with_text_glow` too, the dilated
  mask is blurred: a `text-shadow` of a stroked glyph. Atlas key `BlurredGlyph(params, σ bits,
  radius bits)`.
- `Window::with_text_animation(Some(&TextAnimation), |window| …)`: a CSS animation of the glyphs and
  SVG masks drawn inside, stored per sprite next to its gradient (`Scene::glyph_animations`, same
  index, `Arc`, copied by `replay`). Tracks: `translate`, `scale` around `origin` (CSS
  `transform-origin`), `shift` (gradient `background-position`), `tint` (fill colour), `opacity`
  (multiplies the sprite opacity). The `CubicBezier` timing function applies to each interval
  between two keyframes of a track, like CSS. `delay` has no backwards fill; after the last
  iteration the last keyframe stays (`fill-mode: forwards`); `looped` = `infinite`. A glyph with an
  animation and no gradient gets a one-stop gradient of its colour so it takes the gradient
  pipeline.
- `Scene::glyph_sprite_now` applies scroll and animation at `Scene::gradient_time`: the sprite gets
  its transformation (`scale` around the origin, then `translate`), the gradient moves with the
  translation plus `shift`, `tint` replaces the stop colours. `write_gradient_sprites` writes that
  sprite and gradient. `Scene::scrolls_gradients` is also true while an animation runs, so `present`
  keeps re-presenting without notifying, laying out or painting (patch 17 path).
- `Window::paint_svg` honours `with_text_gradient` and `with_text_animation`: an SVG alpha mask is a
  gradient sprite like a glyph (role dot).
- `MAX_GLYPH_GRADIENT_STOPS` 8 → 12 (the TOON gradient has 10 stops); the WGSL array follows.
- `Window::set_gradient_elapsed` also counts animation starts.

Tests: `text_system::glyph_blur::tests::dilation_grows_the_glyph_by_the_radius_with_an_antialiased_edge`,
`dilation_keeps_partial_coverage_of_the_edge_pixels`; `text_system::text_animation::tests`
(`cubic_bezier_matches_the_css_curves`, `keyframes_ease_every_interval_and_hold_the_ends`,
`progress_waits_for_the_delay_then_loops_or_stops_forwards`,
`apply_scales_around_the_origin_and_moves_the_gradient_with_the_glyph`,
`tint_replaces_the_fill_and_opacity_fades_it`); wgpu
`headless_renderer_moves_and_tints_animated_glyphs_at_the_scene_time`,
`gradient_sprite_record_matches_the_shader_layout` (68 floats per gradient). App:
`fonts::name_paint::tests` (neon, toon, pop, gummy layers, glow fade, hover clock),
`ui::role_dot::tests`, `fonts::tests::effect_palettes_follow_the_client_lightness_steps`,
`neon_flicker_is_the_main_hue_more_saturated_at_85_lightness`, `neon_toon_pop_and_gummy_get_their_own_fills`.

**CPU, 2026-11-26 (demo).** `screenshot` `PHX_DM=900 PHX_SCALE=1.5 PHX_MOUSE=202,646 PHX_PRESENT_CPU=10`
(GUMMY row of the demo DM list hovered, looping), under `phoenix-nice`, frame requests every 16 ms, two runs:

| phase | CPU (one core) | frame requests in 10 s |
|---|---|---|
| hovered GUMMY name animating, present only (patch 18) | 57.0 % / 57.2 % | 268 / 267 |
| same, full redraw every frame | 63.9 % / 63.6 % | 225 / 226 |
| idle, mouse gone, nothing animates (`scrolls_gradients` false, no re-present) | 52.6 % / 55.3 % | 296 / 279 |

As in patch 17 the floor is the harness. An animated name costs 2 to 4 points over idle against
about 9 for a full redraw per frame, and nothing runs once the mouse leaves. Frames at
0 / 200 / 640 / 880 / 1120 / 1600 / 2200 / 3200 ms for the four effects in
`/mnt/windata/project-phoenix/compare/name-effects` (`sheet-*.png`, demo): neon flickers at 640, 880
and 1120 ms (16, 22, 28 %), toon scrolls down its band between 200 and 2200 ms, pop bounces and its
shadow gradient moves over the first 2 s, gummy letters squish one after the other.

Differences left: the stroke is a round dilation of the antialiased mask, where Skia strokes the
outline with miter joins, so sharp corners are slightly rounder; the TOON colour and opacity
transitions on hover end (`transition: color/opacity 266ms`) are not played, because the effect
switches to PLAIN at the same time. The scale of a GUMMY letter stretches its rasterised mask,
like a CSS transform of an already painted layer.

## 19. Smooth wheel scrolling (`src/smooth_scroll.rs`, `src/elements/div.rs` `paint_scroll_listener`, `src/elements/list.rs` `paint` / `StateInner::scroll_by`, `src/window.rs`)

Branch `gpui-smooth-wheel`, rev `cbcafa33`.

**Problem.** Every wheel notch moved the content by its whole distance on the next frame, so
scrolling with a mouse wheel felt "snappy" next to the client. The client runs on Chromium,
which animates each notch on Linux (smooth scrolling is on by default; the Electron command
line of Discord 1.0.171 has no `--disable-smooth-scrolling`, checked in `/proc/<pid>/cmdline`).

**Reference.** Chromium `cc/animation/scroll_offset_animation_curve.cc`, the curve used for
`ScrollType::kMouseWheel` when the Windows "impulse" personality is off (the Linux default):
`DurationBehavior::kInverseDelta`, segment duration `clamp(14 − |Δ|/60, 6, 12)` frames at
60 Hz, that is 200 ms up to 120 px, 100 ms from 480 px, linear in between; timing
`cubic-bezier(0.42, y1, 0.58, 1)` with `y1 = 0.42 × slope` (`EaseInOutWithInitialSlope`,
ease-in-out from rest); a notch during an animation retargets it (`UpdateTarget`): the new
segment starts at the current offset with the current velocity as initial slope, and its
duration is the smaller of the segment duration and `2.5 × remaining / velocity`. A
synthetic `Input.dispatchMouseEvent` wheel in a headless Chromium 157 scrolls in one step,
because DevTools wheels count as precise deltas, so the curve could not be sampled there;
the constants above come from the Chromium source.

**Patch.**
- `gpui::set_smooth_wheel_scroll(bool)` (off by default, so upstream tests keep their
  immediate scrolling). Phoenix turns it on in `phoenix_app::init`, `PHX_INSTANT_WHEEL=1`
  turns it off for comparisons.
- Only `ScrollDelta::Lines` from `ScrollSource::Pointer` is animated: Wayland and X11 report
  a wheel notch as 3 lines. Touchpads (`ScrollDelta::Pixels`) and touch pans keep moving the
  content directly. Under X11, the XI2 smooth scrolling of a touchpad also comes as fractional
  lines, so it is animated too.
- The distance of a notch does not change: `3 × window.line_height()` in a div scroll
  container (and `uniform_list`), `3 × 20 px` in a `list`.
- `Window::smooth_wheel_scroll(key, delta, apply)` keeps one `WheelCurve` per container (`key`
  = address of its scroll offset `Rc`, or of the `ListState`). It is stepped from a next-frame
  callback, which the platform delivers on the compositor frame callback, so the steps follow
  the display rate. Each step calls `apply` with the part of the delta for that frame.
- A div container adds that part to its scroll offset and notifies its view; the offset is
  clamped in prepaint as before.
- A `list` scrolls by that part from its current logical scroll top (`StateInner::scroll_by`),
  so items spliced in while the animation runs (older history, new messages) keep their anchor.
  It goes through `StateInner::scroll`, so a `ListAlignment::Bottom` list that reaches its end
  drops its anchor and follows the tail again, and the scroll handler (Phoenix's history
  loading) is called on every step. A step that lands within 0.05 px of the end snaps to it,
  so float rounding cannot leave the chat one hair above the bottom.

**Fixes, 2026-11-26 evening (branch `gpui-scroll-fix`).**
- The chat did not move with the wheel. A `ListAlignment::Bottom` list resting at its end has
  no anchor, so its logical top is past the last item; `StateInner::scroll_by` started from that
  height and every step was clamped back to the end. It now starts from the offset the list is
  drawn at (`min(scroll_top, scroll_max)`). Test `test_smooth_step_scrolls_a_bottom_list_resting_at_its_end`.
- Notch distance: `wheel_line_delta` gives 40 DIPs per line divided by the page zoom (120 DIPs a
  notch). Measured on Chrome 158 under Xvfb with real X11 wheel clicks (`xdotool click 4`),
  device scale 1.5, page zoom 1: 120 CSS px a notch. In the client (Wayland scale 1, dpr 1.5)
  a CDP wheel of 100 DIPs scrolls 66.7 CSS px, so its deltas are divided by the 1.5 zoom:
  one notch is 80 CSS px, 120 physical px. Before, a notch was 60 px in a list and
  `3 × line_height` in a div.
- Inactive windows: GPUI throttles an unfocused window to 30 fps (`inactive_frame_interval`),
  so a wheel over Phoenix while another window had the focus animated every other frame. The
  frame handler no longer throttles while `Window::smooth_scrolling()`.
- `ListState::smooth_scroll_by` and `ScrollHandle::smooth_scroll_by` run a scroll on the same
  animation (same key) as the wheel; Phoenix's scrollbar track clicks use them.
- Measured curve (Chrome 158, rAF `scrollTop`): one notch of 120 px eases in and out over
  175–200 ms; 10 notches sent 25 ms apart reach their 1200 px target 300–370 ms after the
  first, without a pause between notches, which is the retarget already implemented.

**Page scroll, 2026-11-27 (branch `gpui-scroll2`).**
- Neither the scroll-fix commit above nor the PageUp fix `e9d7972e` was ever built: `Cargo.toml`
  pins GPUI to a git rev (`4a3f89ec`), and both changes only went into `vendor/` on branches
  whose pin never merged. So `real_shots` kept showing `list_top=(51, 0)` after a PageUp from the
  bottom: `ListState::scroll_by` started from the past-the-end anchor (`content height`), moved
  less than a viewport, and the next layout found the bottom of the list visible again and
  dropped the anchor (`ListAlignment::Bottom` → `set_scroll_anchor(None)`). Both fixes are now in
  the pinned rev.
- `ListState::scroll_by_notify(distance, view, window, cx)` scrolls now from the drawn offset
  through `StateInner::scroll`, like one wheel step: the scroll handler runs (history loading,
  « Revenir au présent »), the end re-attaches the tail. Phoenix's PageUp/PageDown spring calls it
  once per frame. Test `test_scroll_by_notify_runs_the_handler_and_follows_the_tail_at_the_end`.

Tests: `smooth_scroll::tests` (segment ramp, ease-in-out of one notch over 200 ms, second notch
keeps the velocity, reversing restarts from the current offset, a notch after the end starts a
fresh segment).

## 20. Image sequences played at present time (`src/scene.rs` `ImageSequence` / `insert_sequence_sprite` / `polychrome_sprite_now`, `src/window.rs` `paint_image_sequence`, wgpu `write_instances`)

Same branch and rev as patch 19.

**Problem.** The Lottie hover icons (mic, headset, settings, the voice and composer buttons)
called `request_animation_frame` from their image closure on every frame of their segment.
That notifies the view, and Phoenix is one view, so every frame of a 40 to 70 frame segment
laid out and painted the whole window. Any frame that took longer than the refresh interval
was dropped, which is the stutter seen on the mic and headset buttons.

**Patch.**
- `Window::paint_image_sequence(bounds, radii, sheet, columns, count, start, fps, phase)`
  uploads `sheet` (all the frames of a segment in a grid, one atlas tile) and paints one
  `PolychromeSprite` whose tile is the first cell, with an `ImageSequence` next to it
  (`Scene::sprite_sequences`, index in `pad`, copied by `replay` like the glyph gradients).
- At present time the wgpu renderer writes each polychrome sprite through
  `Scene::polychrome_sprite_now`, which moves the tile to the cell of
  `floor((gradient_time − start) × fps + phase)`, held on the last frame.
- `Scene::scrolls_gradients` is also true while a sequence has frames left, so `present` keeps
  re-presenting through the present-only branch of patch 17: nothing is notified, laid out or
  painted until the hover ends or something else changes.
- `Window::set_gradient_elapsed` also counts sequence starts, so `real_shots`
  `PHX_GRADIENT_AT` records a playing icon frame by frame at the present-time cell.

Tests: `scene::image_sequence_tests` (index rounding and hold, the tile moves to the cell of
the frame, replay keeps the sequence). App: `ui::hover_icon::tests::sheet_cells_follow_the_player_frames`
(the cell picked at present time is the frame the player computes, at 0, 7, 16, 100, 333,
650 and 2000 ms).

## 21. Cut-outs (`src/scene.rs` `CutoutSet` / `push_cutouts` / `replay`, `src/window.rs` `with_cutouts` / `paint_quad` / `paint_image` / `paint_emoji`, `src/color.rs` `Background::with_cutout`, wgpu `shaders.wgsl` `cutout_coverage`, `wgpu_renderer.rs` binding 2 of the globals group)

**Problem.** The client cuts the guild icon around its badges with an SVG `<mask>` (BlobMask,
module 862328): a black rounded rect 2 px larger than the badge leaves a transparent ring between
the icon and the badge. Phoenix painted a ring in the rail colour over the icon instead, which
showed the wrong colour over an open folder, while dragging, and around avatars on a hovered row.
GPUI has no mask.

**Patch.** `Window::with_cutouts(&[(bounds, radius)], f)` punches up to two rounded-rectangle
holes, in window coordinates, through the quads and images (polychrome sprites, emoji) painted by
`f`. The set (`CutoutSet`, two `CutoutHole`s in device pixels) goes into `Scene::cutouts`, and the
primitive carries `1 + index` in a field that was padding: `Background::pad` for quads and
the high 16 bits of `PolychromeSprite::pad` for images (the low 16 bits keep the image sequence of patch 20, `poly_sequence` / `poly_cutout`), so no instance layout changes (the WebGL decoders already read
those words). `Scene::push_cutouts` reuses the last set when it is the same and returns 0 once
`MAX_CUTOUT_SETS` (256) sets are stored; `replay` copies the set of a reused primitive. The wgpu
renderer writes the sets to a 16 KiB uniform (`@group(0) @binding(2) cutouts`) after the gamma
params when the scene has any. `fs_quad` (now a wrapper of `fs_quad_uncut`, the index comes as a
flat varying) and `fs_poly_sprite` multiply their alpha by `saturate(distance + 0.5)` per hole,
the signed distance of a rounded rectangle, the same half-pixel ramp as a rounded quad. The
radius is clamped to half of each side on its own (`radius`, `radius_y`), like SVG `<rect rx>`
without `ry`: the BlobMask lower hole of a folder with 10+ mentions (26 × 20, rx 12) keeps
12 × 10 elliptical corners, whose distance is `(|q / r| − 1) · min(r)`, exact for a circle.
Monochrome sprites (glyphs and SVG icons: the Home logo, the star of Favorites, guild acronyms)
are cut too since wt/voice-lobby2: `MonochromeSprite::pad` uses the same split as images (low 16
bits = glyph gradient index of the text gradient patch, read through `poly_sequence`; high 16 bits
= cut-out set), `paint_glyph` / `paint_svg` stamp `self.cutouts`, `replay` remaps the set, and
`vs_mono_sprite` passes `pad >> 16` as a flat varying that `fs_mono_sprite` multiplies into its
alpha. This replaced the ring painted in `BG_TERTIARY` over the holes (959326aa), which showed a
black ring over an open folder; in the live app the logo still crossed the ring because the hole fill and the logo were not always drawn in tree order (present transitions of the hovered tile), while a cut on the sprite itself does not depend on the order. Gradient text, paths, shadows and underlines are not cut. A
scene without cut-outs takes the old path.
Search `PHOENIX PATCH (cut-outs)`.
The mono sprite cut is pinned at 29af93c0 (branch `gpui-mono-cutout`, on top of the page scroll pin 2143fc3c). The pinned commit (4a3f89ec, the present transitions commit 4291d676 plus the per-axis radii) is kept on the `gpui-cutout-radii` branch of origin.

Tests: `scene::cutout_tests` (circle and pill distances, per-axis corners, empty hole, set reuse, full table, clear),
`scene::image_sequence_tests::sequence_and_cutout_share_the_pad_word`,
wgpu `headless_renderer_cuts_round_holes_through_quads` (round and pill holes, a neighbour quad
untouched), `headless_renderer_cuts_round_holes_through_monochrome_sprites` (fails before the mono patch), and the existing WGSL validation tests for the storage, gradient, subpixel and WebGL
shaders. App: `ui::blob_mask::tests`.

## 22. Present-time transitions and exit ghosts (`src/present_transition.rs`, `src/scene.rs` `transitions` / `*_transitions` / `ExitGroup` / `adopt_exits` / `replay`, `src/window.rs` `with_present_transition` / `with_exit_transition` / `draw`, wgpu `write_instances` / `write_gradient_sprites` / path batches)

**Problem.** Popouts, modals and the settings layer opened without the client's animation, and
every hover fade, tooltip zoom, rail pill and the composer emoji spring went through
`with_animation` or `request_animation_frame`. Phoenix is one view, so each frame of a 100 to
600 ms transition laid out and painted the whole window (the defect patch 20 removed for Lottie).
A closing modal could not animate at all: once the app drops it, nothing is painted.

**Patch.**
- `Transition` (CSS-like, logical px): `start` (or the first paint of the element), `delay`,
  timing `Ease { duration, CubicBezier }` or `Spring(SpringConfig)` (analytic damped spring from
  rest, under, critically or over damped; it ends when it is within 0.001 of its target), and
  from → to tracks: `translate`, `scale` around `origin` (a fraction of the element bounds),
  `opacity`, `background` (solid quad colours, mixed in premultiplied sRGB like CSS) and `height`
  (quads resized around their middle, corner radii kept). `keep_masks()` leaves the content masks
  in place (a text roll inside a clipping box); by default masks move with the content, so a
  scrolled popout slides with its clip.
- `Window::with_present_transition(Some(&t), bounds, start, f)` stores a `SceneTransition`
  (device px, `gradient_clock` time, parent index for nesting) and tags every primitive inserted
  by `f`. `Scene::insert_primitive` pushes the tag into one `Vec<u32>` per primitive kind and one
  per paint operation; `Scene::finish` sorts each kind with its tags (`sort_tagged`, the old
  `sort_by_key` when no tag is set). A transition that is over and leaves everything as painted
  is not stored, so a settled UI keeps the old fast path.
- At present time the wgpu renderer resolves every chain once (`Scene::transition_states`, inner
  then outer, `TransitionState::then`) and writes moved, scaled, faded or recoloured copies of
  quads, shadows, underlines, mono, gradient, subpixel and polychrome sprites and paths. Sprites
  with their own transformation get the transition composed into it. `Scene::scrolls_gradients`
  is also true while a transition runs, so the present-only branch of patch 17 draws the frames:
  nothing is notified, laid out or painted.
- `Window::with_exit_transition(key, &exit, bounds, f)` records what `f` paints as an
  `ExitGroup`. At the end of `draw`, `Scene::adopt_exits` replays every group of the previous
  scene whose key was not painted again (and every ghost still running) with `exit` as the root
  of its chains, started at that frame. Ghosts have no hitboxes. A group inside another vanished
  group goes with the outer one. `Scene::replay` (view caching) copies tags and groups.
- `PresentTransitionExt::present_transition(id, Option<Transition>)` wraps an element (the start
  is kept in its element state), `.on_exit(key, exit)` adds the exit group.
- `Window::set_gradient_elapsed` also counts transition starts, so `real_shots`
  `PHX_GRADIENT_AT` records an opening frame by frame.

Tests: `present_transition::tests` (ease-out slide with its delay and end, springs at rest /
overshoot / settle, scale around the origin and faded colours, premultiplied mix, nested chains
and fixed masks, quad height). App: `ui::popout_motion::tests`.

## 23. Wayland clipboard keeps the image and the text (`vendor/gpui-pre-linux`, `src/linux/wayland/clipboard.rs` `both_entries`)

Contributed by Rodrigue Neveux (PR #17).

**Problem.** `gpui-pre-linux` read the clipboard text first and only fell back to the image.
A browser that copies an image also offers its URL or HTML, so pasting it in the composer
pasted the text and lost the image. The client (Chromium `clipboardData.files`) attaches the
image.

**Patch.** `read_from_clipboard` and `read_from_primary` read both offers and return one
`ClipboardItem` with the image entry first and the text entry after it. A source that offers
only one of them is unchanged. `ChatState::paste_clipboard` already uploads image entries and
consumes the paste when it took one.

`vendor/gpui-pre-linux` is the crates.io `gpui-pre-linux` 0.3.12 source with only this patch
(marked `PHOENIX PATCH (clipboard)`). It is wired through its own git rev, the
`gpui-linux-clipboard` branch, so the other GPUI crates keep their rev and are not rebuilt.
After changing it: commit it on that branch, push, and set the `gpui-pre-linux` rev in
`Cargo.toml` and in `Cargo.lock`. X11 is not patched.

## 24. Deferred elements keep their image cache (`src/window.rs` `DeferredDraw::image_cache_stack`, `defer_draw`, `prepaint_deferred_draws`, `paint_deferred_draws`)

**Problem.** `deferred()` children are laid out inside the tree but prepainted and painted after it,
when `Window::image_cache_stack` is empty. Every `img()` in a popout, menu or modal drawn through
`deferred()` (including Phoenix's `media::lru::deferred`, which wraps the child in the LRU cache)
therefore resolved its image in the LRU at layout and again through `use_asset` at prepaint and paint.
The second copy lives in GPUI's global asset map, which never evicts: each avatar, banner or
decoration seen in a profile popout stayed decoded in memory for the rest of the session.

**Patch.** `defer_draw` records the image cache stack with the element id and text style stacks,
and the deferred prepaint and paint restore it, then clear it. Rev: `617acfac` on
`gpui-deferred-cache` (parent `29af93c0`, so nothing else moves).

## 25. Fallback faces picked by CSS weight matching (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `fallback_attributes`, `css_matched_weight`)

**Problem.** In a gg sans 600 friend row, « مادارا » was drawn in Noto Sans Arabic Bold: heavier and
wider than the client, so the name ellipsized one Hangul syllable earlier (« 마… » instead of « 마다… »)
and left ~16 CSS px before the server tag.

**Reference.** Live client 2026-11-28, `CSS.getPlatformFontsForNode` on a hidden `var(--font-primary)`
16px span: DejaVuSans (Arabic) and NotoSansCJKjp-Regular (Hangul) at 400, 500, 600 and 700, widths
unchanged (« مادارا » 37.19 px = HarfBuzz DejaVu Sans Book 37.188; DejaVu Bold would be 44.31,
Noto Sans Arabic Bold 37.55). Ink grows from 600 (synthetic bold, patch 11).

**Cause.** cosmic-text `FontFallbackIter::default_font_match_key` keeps a family face only when
`font_weight_diff == 0`. The fallback `Attrs` carried the primary weight (600), so every family without a
600 face (`Phx DejaVu Sans`, `Phx Noto Sans` 400/700, the CJK families) was skipped and cosmic-text went on
to its own script fallback (Noto Sans Arabic, which has a Bold).

**Patch.** `load_family` records the weights and styles of each fallback family's faces;
`fallback_attributes` asks each family for the weight CSS font matching picks among them
(`css_matched_weight`: 400–500 look up to 500 then down, below 400 down then up, above 500 up then down,
same style first). Synthetic bold still applies when the run asks for 600+ and the face is lighter. Rev
`112acf8d` (branch `gpui-fallback-weight`, on top of `869f38c1`). Test
`fallback_weight_follows_css_font_matching`.

## Running the vendored tests

The lib tests of both crates run from a small test-only workspace, `vendor/Cargo.toml`
(members `gpui-pre` and `gpui-pre-wgpu`, `gpui-pre` patched to its path). Its
`vendor/Cargo.lock` started as a copy of the main lockfile, so the shared target dir reuses
the same dependency builds:

```sh
flock /tmp/phoenix-build.lock env CARGO_TARGET_DIR=$HOME/discord-phoenix/target CARGO_PROFILE_DEV_DEBUG=0 \
  cargo test --manifest-path vendor/Cargo.toml -p gpui-pre -p gpui-pre-wgpu --lib --features gpui-pre/test-support
```

The integrator runs this after the Phoenix tests, but only for a batch that touches `vendor/`.
In the shared target, the test build is a third variant of gpui, and the pruning in
`phoenix-disk-guard` keeps only two per crate. Running it on every batch would evict the Phoenix variant and
rebuild gpui and everything above it.

Cost, measured on 2026-11-25: first build 4 min 40 s and about 1.4 GB in `target/`, then
30 s to run (436 tests passed in gpui-pre and 48 in gpui-pre-wgpu; 2 ignored by upstream,
`frame_cost` and `hash_cost`, which are timing probes). Every patch test passes, so nothing
in the patches had to be fixed.

The crates.io packages ship without dev-dependencies or test files. What was restored, from
zed@d2a4fb6 (the snapshot named in `[package.metadata.gpui-pre]`):
- dev-dependencies: `util_macros` = crates.io `gpui_util_macros` 0.2.2 (Zed's crate, published,
  for `#[perf]`); `naga` 30.0.2 with `wgsl-in` (the version wgpu 30 uses) and `image` (png)
  for gpui-pre-wgpu;
- `vendor/test-assets/fonts/{lilex,ibm-plex-sans}`: Lilex Regular/Bold and IBM Plex Sans
  Regular, SIL OFL 1.1 (license files alongside). The `include_bytes!` paths in
  `svg_renderer.rs` and `cosmic_text_system.rs` point there instead of Zed's top-level `assets/`;
- `vendor/gpui-pre/examples/image/exif-orientation-rotate-180.jpg` (Apache-2.0, like the crate).

`vendor/Cargo.toml` does not affect the main build. A git dependency on a rev that contains it
still resolves (checked with `cargo metadata` against a `file://` rev).
