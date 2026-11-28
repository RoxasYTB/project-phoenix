# Fonts (gg sans, gg mono, ABC Ginto) and their fallback stacks

Sources: the `@font-face` rules and the `--font-*` variables of the client CSS (`~/discord-phoenix-ref/current-client/css/`, 2026-11-26), the computed `font-family` of every text node in the dumps (`/mnt/windata/project-phoenix/dumps`, `compare/settings-c/cli`), the cached JS (module 142893, `StreamPreview`). The font files come from the local Discord cache or `discord.com/assets/<hash>.woff2` and are written to `~/.cache/discord-phoenix/fonts/`; they are never committed (`no_font_files_in_repo`).

## Stacks (client CSS, Latin locales)

| variable | stack |
|---|---|
| `--font-primary` | "gg sans", "Noto Sans", "Helvetica Neue", Helvetica, Arial, sans-serif |
| `--font-code` | "gg mono", "Source Code Pro", Consolas, "Andale Mono WT", "Andale Mono", "Lucida Console", "Lucida Sans Typewriter", "DejaVu Sans Mono", "Bitstream Vera Sans Mono", "Liberation Mono", "Nimbus Mono L", Monaco, "Courier New", Courier, monospace (6 identical rules in `current-client/css`) |
| `--font-nitro` (`nitro-*`, `display-*` text variants) | "ABC Ginto Discord Nord", "Noto Sans", "Helvetica Neue", Helvetica, Arial, sans-serif |
| `--font-headline` | "ABC Ginto Nord", "Noto Sans", … |
| `--font-display-marketing` | "ABC Ginto Discord", "gg sans", serif, "Noto Sans", … |
| `--font-display-marketing-header` | "ABC Ginto Discord Nord", "gg sans", serif, "Noto Sans", … |

"Noto Sans" in these stacks is the web font of the client CSS (`@font-face` Noto Sans 400–800, normal and italic), not the system one.

## Faces declared by the client

gg sans 400, 500, 600, 700, 800, each normal and italic; gg mono 400 and 700; ABC Ginto Discord 400, 500; ABC Ginto Discord Nord 700, 700 italic, 900 italic; ABC Ginto Nord 800, 800 italic. The dumps only show gg sans at 400–700 normal, gg mono 400/600/700 (600 takes the 700 face) and Ginto Discord Nord 700 (Nitro pages).

## What Phoenix did wrong, and the fix

| problem | effect | fix | status |
|---|---|---|---|
| `fonts::register` returned early once gg sans and one Ginto face were on disk | faces cached later by the client (gg sans 500 italic, 700 italic) were never extracted; GPUI drew them with a neighbouring face | the extraction always runs in the background; `install` is keyed by face (`gg_sans-500i`), not by family, so new faces of a known family are added | ✅ |
| faces declared in the CSS but absent from the cache (gg sans 800 / 800 italic, Ginto Discord 400/500, Ginto Discord Nord 700i/900i, Ginto Nord 800/800i) | `font-weight: 800` and Nitro italic titles fell back to another weight or a synthetic style | `fonts::extract` returns the missing `(file, asset)` pairs and `download` fetches them by GET from `discord.com/assets/` like the client does | ✅ |
| gg sans 600 text with a glyph from a regular-only fallback (Arabic in DejaVu Sans, Hangul in Noto Sans CJK) | cosmic-text skipped every fallback family without a 600 face and drew Arabic in Noto Sans Arabic Bold (heavier, wider), so friend names ellipsized early | GPUI patch 25: each fallback family is asked for its CSS-matched weight, then synthetic bold from 600 as in Chromium (live `getPlatformFontsForNode` 28/11: DejaVuSans, NotoSansCJKjp-Regular at every weight) | ✅ |
| GPUI tooltips (`.tooltip(tooltip::tip(..))`, 60 callers) are prepainted as window roots with an empty text style | their text used `.SystemUIFont` = fontconfig sans-serif = system Noto Sans | `Tip` starts with `fonts::ui_root` (family + fallback chain) | ✅ |
| gpui-component `Root` surface and its tooltips use `Theme::font_family` (`.SystemUIFont`) | the call popout window (its own `Root`) and gpui-component tooltips (voice panel ping, E2EE) were in Noto Sans | `fonts::apply_theme_fonts` sets the theme's `font_family` / `mono_font_family` to gg sans / gg mono after every install; the popout root also uses `ui_root` | ✅ |
| Nitro titles (`nitro-xs/sm/md`, `nitro.hello`, Nitro home section titles, Nitro Squad card, Nitro settings header) | drawn in gg sans 800 italic | `fonts::nitro(el, italic)`: Ginto Discord Nord 700 (italic for `nitro-*`), as in `.nitro-md_RAOoio{font-family:var(--font-nitro);font-style:italic;font-weight:700}` | ✅ |
| stand-in when a display face is missing | always gg sans | `display_stand_in`: gg sans only for the marketing stacks, which list it; Nitro and headline go to the web Noto Sans like the client | ✅ |
| U+202F (narrow no-break space, French number formatting `1 234`) | gg sans has no glyph for it | the chain takes the web Noto Sans for that one character, as Chromium does with `--font-primary` | ✅ (same as the client) |

## Coverage check

`fonts::tests::ui_text_resolves_to_the_client_faces` loads the faces from the local cache, checks that every face of gg sans, gg mono and ABC Ginto has a glyph for a FR/EN set (accented capitals and lower case, œ æ ÿ, « » ’ “ ” … – — • € °, no-break space, digits, ASCII punctuation), lays the set out with the real cosmic-text system at every declared weight and style and asserts that no glyph leaves the requested face, that two weights never share a face, and that U+202F goes to the web Noto Sans. `ui_text_never_names_a_font_directly` rejects any literal family outside `fonts/` and checks that the window-level views start from `ui_root`. The test does nothing on a machine without the Discord cache.

## Settings pages

Line widths in `real_shots PHX_OPEN=settings:<page>` already matched the client within 1 device px (Data and privacy, Notifications at dpr 1.5): the settings text was gg sans before this change. The wrapping difference left on "Utiliser des données tierces…" ("les" moves to the next line) is a sub-pixel width difference, not a font change.

## `--font-code` without gg mono

When gg mono is not in the Discord cache, `fonts::mono_family` walks `fallback::CODE_STACK` (the client stack above, minus gg mono and `monospace`) and keeps the first family that fontconfig returns under its own name (`fc-match "<name>"` → `%{family[0]}` equal to the name, so an alias such as Consolas → DejaVu Sans Mono does not count), then `fc-match monospace`, then DejaVu Sans Mono. Chromium takes the first installed family of the list the same way. On this machine: Source Code Pro → Noto Sans (absent), Consolas → alias (absent), so DejaVu Sans Mono, as before. Unit test `code_stack_follows_the_client_order`. ✅

