# Expression pickers (emoji / GIF / sticker)

Source: dumps `vanilla-picker-emoji`, `vanilla-picker-gif`, `vanilla-picker-sticker` (1280x700 viewport) + CSS bundle 569603.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| picker body | emojiPicker_-sZhZn (#2502) | size | 497x427 | pickers/mod.rs PICKER_W | ✅ |
| nav tabs | nav_58b0l2 (#2495) | height / padding | 30 / 0 16 | pickers/mod.rs tab_button | ✅ |
| search | container_VhFsFg (#2504) | size | 272x40 | pickers/mod.rs search_box | ✅ |
| search focused | live GIF picker 2026-11-25 (`wrapper_UcGqzv container_0PLWhS md_`) | border | focused on open: 0.8px border + 1.6px outline, brand (--input-border-active), radius 8, 464.4×40 | search_box: 2px --input-border-active | ✅ (was 1px --border-focus, grey) |
| diversity | diversitySelector_eHzdRB (#2511) | size | 38 | grid.rs tone_button | ✅ |
| emoji cell | emojiItemMedium_6OcK5D | size / img | 48 / 40 | grid.rs render_row | ✅ |
| rail | wrapper_6AvvhR (#2697) | width | 48 | grid.rs rail | ✅ |
| rail Nitro row | row_3MXiRB (#2701) | border-bottom / pb / mb | 1px --border-subtle / 8 / 8 | grid.rs render_body | ✅ (non-Nitro only) |
| rail pill | .small__o8_Xq .item.selected | height | 24px, on default categories too | grid.rs rail_pill | ✅ |
| frequently used | categorySection (#2525), 42 items | source | settings-proto/2 emoji frecency | grid.rs build + rest/frecency.rs | ✅ order: see below |
| top server emoji | `TOP_GUILD_EMOJI` section (`categorySection_6c9R7U`), first section | source / count | `GET /guilds/{id}/top-emojis` → `items[{emoji_id, emoji_rank}]` sorted by rank, cached one day (`topEmojisTTL`), fetched for the current server only, never for reactions; row = `top.slice(0, 9 − newly)` + up to 3 emojis of the server created in the last 60 days (`fromTimestamp(now − 60 d)`, newest first) | grid.rs top_section + pickers::load_top_emojis | ✅ live (9 items, first one selected) |
| top section header / rail | `header_kSdzCr`, `TrophyIcon` | icon | 16px trophy before "Émojis populaires sur <serveur>" (body-sm/semibold), 24px trophy in the rail before the clock | grid.rs section_icon, render_body | ✅ live |
| POPULAIRES badge | `topGuildEmojiBadge_1bDrGE` | box | padding 4 8, gap 6, radius 4, `linear-gradient(268.26deg, rgb(191,81,81), rgb(219,128,63) 102.45%)`, 14px fire + body-sm/semibold uppercase white "Populaires"; only for the top (not newly added) items | grid.rs popular_badge | ✅ live; the client's flame is an inline 10×13 path, Phoenix uses the extracted `FireIcon` |
| newly added sub-block | `NEWLY_ADDED_EMOJI` | | unseen newly added emojis get their own block and an "Ajouté(e) récemment" badge in the client | grid.rs render_row, prefs.rs `newly_seen` | ✅ wt/fin-chat: CSS `31544.*` (`*_6OcK5D`): unseen ones (NewlyAddedEmojiStore `isNewerThanLastSeen`: no record, newer id, or < 2 weeks since last seen and not acknowledged) after the others in a 1px #2d7d46 box (radius 4 4 4 0 / 4 4 0 when after other emojis / 4 4 0 0 for one), 50×16 badge below (#2d7d46, radius 0 0 4 4, SparkleIcon 12 + "NOUVEAU" body-xs/semibold), 50 % alpha on hover, row margin-bottom 20; on close the picker commits the pending state then marks the newest newly added emoji pending (`uV`); hovering one ≥ 250 ms acknowledges it (`mz`); state kept in `expressions.json`. Inspector: "AJOUTÉ(E) RÉCEMMENT" badge, gradient 268.26deg #097d8d → #60a654, sparkle 14 (`newlyAddedBadge_1bDrGE`) |

### Frequent emoji and sticker order (frecency)

Source: `web.b63d846b329d6470.js` module `283047` (the generic `Frecency` class), the emoji store
(`eA` / `ef`, `numFrequentlyItems: 42`, `computeBonus: () => 100`) and the sticker store
(`numFrequentlyItems: 20`). Data: `settings-proto/2` (`FrecencyUserSettings`), `emojiFrecency`
(field 6) and `stickerFrecency` (field 4), read only.

- For each entry, score = Σ over the first 10 `recentUses` of `weight(days)` with
  `days = moment().diff(t, "days")` (truncated) and weight 100 (≤ 3 d), 70 (≤ 15), 50 (≤ 30),
  30 (≤ 45), 10 (≤ 80), 1 beyond; `frecency = ceil(totalUses × score / recentUses.length)`.
  Entries without recent uses are dropped. The stored `frecency` / `score` fields are ignored.
- Sort: stable by descending frecency, ties in JS object key order: canonical integer keys
  below 2³²−1 first (for example the emoji "100"), then the protobuf map order on the wire.
  Phoenix reads that order from the raw bytes (`rest/frecency.rs` `wire_order`) because prost
  maps lose it.
- Then unknown keys are skipped and the first 42 emojis / 20 stickers kept.
- Empty emoji frecency: the client seeds thumbsup, eyes, laughing, watermelon, fork_and_knife,
  yum, weary, tired_face, poop, 100 (so "100" comes first).
- Reactions use another history (`emojiReactionFrecency`, frecency
  `trunc(1000 × (uses / maxUses × .2 + score / 1000 × .8))`, `maxUses` = largest totalUses of
  the history, score = Σ weights of the first 10 uses): ✅ wt/fin-chat, settings-proto/2 field 13
  (`emojiReactionFrecency`, same message as field 6) and favorites field 5, read only
  (`rest/frecency.rs` `reactions_ranked_at`). Used where the client uses it: the reaction picker's
  frequent row (`EmojiIntention.REACTION` → `emojiReactionFrecency.frequently`), the hover bar quick
  reactions (3) and the message menu strip (4) = favorites then frequent reactions
  (`getEmojiInPriorityOrderWithoutFetchingLatest`), customs of other servers dropped without Nitro.
  Live check 2026-11-25: the client's hover bar shows heart, Keo_Wow, white_check_mark = the top 3
  computed from the proto (Phoenix had a fixed 👍 ❤️ 😂).
- Checked on the real account (2026-11-25): the 42 emojis computed from the live proto match the
  client's "Fréquemment utilisés" section item for item (✅ ❤ then customs, wilted_rose…).

| inspector | inspector_1bDrGE (#2687) | height / graphic | 48 / 28 | grid.rs inspector | ✅ |
| GIF tiles | result_H8nOze | size / gaps | 231x110, 11 / 12 | gif.rs | ✅ |
| GIF favorites tile | img gif_H8nOze under categoryFadeBlurple | image | newest favorite GIF | gif.rs (favorite_gifs) | ✅ |
| GIF trending tile | img under categoryFade | image | first trending GIF | gif.rs | ✅ cause: `/gifs/trending` returns the preview as a raw `static.klipy.com` URL (not proxied), blocked by the image host allowlist until 24dd2a3; checked 2026-11-24 with `PHX_DEBUG_GIF` on real data |
| GIF requests | bundle 3ad442 `GIFS_TRENDING` / `GIFS_SEARCH` / `GIFS_SUGGEST` | query | `media_format` + `locale` (+ `q`, `limit`), no `provider` since Klipy | rest/search.rs gif_query | ✅ |
| GIF search placeholder | intl `T1Frnm` | text | "Search Klipy" / "Rechercher sur Klipy" | picker.search_klipy | ✅ |
| HC borders | `.high-contrast-mode .navButton_58b0l2` / `.navButtonActive_` / `.result_H8nOze` / `.wrapper_6AvvhR` / bare `.wrapper_-sZhZn` | border | 1px --border-subtle / --border-normal / --border-strong / inline-end --border-subtle / --border-normal | mod.rs tab_button, gif.rs hc_outline, grid.rs rail_shell | ✅ CDP computed (iframe) |
| Light selected emoji | `.theme-light .emojiItemSelected_6OcK5D` | bg / shadow | --background-base-low, inset 1px --border-subtle | grid.rs render_row | ✅ CDP computed |
| locked sticker | `.stickerLocked_5z8c5P` | filter | brightness(.4); Light opacity .4 | sticker.rs cell (dark: opacity .3 approximation, GPUI has no filter) | ✅ |
| sticker cell | sticker_5z8c5P | size / gap | 98 / 13 | sticker.rs | ✅ |
| sticker frequent | stickerCategoryGeneric (#2731) | rail + section | clock, first | sticker.rs | ✅ |
| soundboard | official guide + local client bundle `web.b63d846b329d6470.js` + CSS `967850.025085515c98e619.css` | read path + picker | default sounds: `GET /soundboard-default-sounds` (`RestClient::soundboard_default_sounds`); guild sounds: Gateway op 31 `REQUEST_SOUNDBOARD_SOUNDS {guild_ids}` → `SOUNDBOARD_SOUNDS {guild_id, soundboard_sounds[]}` (`GatewayHandle::request_soundboard_sounds` / `GatewayEvent::SoundboardSounds`). Picker (`ui/pickers/soundboard.rs`): `148×40px` `soundButton_N99eHz` tiles, radius 8, `--background-mod-subtle` / hover `--background-mod-strong` + `1px --border-subtle`, 3-column grid grouped by server + Discord defaults | `phoenix-api` + `ui/pickers/soundboard.rs` | ✅ read-only contract & picker (no voice call, playback, or upload) |

# Search filters: date rows and calendar

Source: live dumps (private) `search-date`, `search-date-operator`, `search-date-calendar`
(2026-11-24, opened from the search popout's "Plus de filtres" row and closed without applying)
+ CSS `calendarPicker_7yAVe6`, `dateFilter_R726Px`. Note: the older `search-add-date` dump
shows the quick switcher, not this modal.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| add date button | button_F7qvhY md secondary, full width | content | 20px plus + "Ajouter une date", 8px under the label, 12px under a row, stays for more dates | search_filters.rs date_section | ✅ |
| date row | dateFilter_R726Px | layout | 40px, gap 8: 120px type select, date field (254 in a 430 column), 40px remove | date_row | ✅ |
| type select | selectField_vWC4bk + listBox_b5PEJH | options | "Avant le…" / "Après le…" / "Le…" (en Before / After / During) → `before:` / `after:` / `during:` | DateOp | ✅ |
| date field | container_tafmhI | style | padding 8 12, gap 8, "24 nov. 2026" 16/20 --text-default, 18px calendar glyph | date_row | ✅ |
| remove | removeDateFilterButton_R726Px | icon | 24px trash, --icon-muted | date_row | ✅ |
| clear count | textButton_nJgowo | text | "Effacer les filtres (n)" once something is set | render | ✅ |
| calendar shell | .calendarPicker_7yAVe6 .react-datepicker | box | 322 wide (280 + 2×20 + border), --background-surface-higher, 1px --border-subtle, radius .3rem, legacy elevation border + high, 8px under the field | calendar | ✅ |
| month title | __current-month | text | uppercase 13/20 semibold --text-strong, padding-bottom 20 + 1px rule (Dark --primary-800 30%), margin-bottom 10 | calendar | ✅ |
| nav | __navigation--previous / --next | box | 18x18 at 20,20, 1px --primary-800, radius 2; no "next" on the current month (future days disabled) | calendar | ✅ (chevron glyph instead of the 6x12 background arrow) |
| day names | __day-name | text | 13px medium uppercase, --primary-100 60%, 40 wide, 10px under; Monday first (Sunday first in English) | calendar | ✅ |
| day cells | __day | box | 40x40, 1px --border-subtle left/top, last column/row closed in --primary-800, corner radius 3 | calendar | ✅ |
| day states | __day--outside-month / --disabled / --today / --selected / :hover | colour | 500 at 30%; disabled on --primary-630; today bold; selected 3px --brand-500 bar; hover --brand-500 + white | calendar | ✅ |
| Escape | — | behaviour | closes the calendar / type list before the modal | search.rs on_key | ✅ |

## Search filters: option lists (live CDP 2026-11-24, 1536×840 @1.25, Onyx)

Measured on the client by opening De / Dans / Contient (screens + `selectDropdown_vWC4bk`
of the date type list in dump `search-date-operator`), closed without applying.

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| list box | selectDropdown_vWC4bk > listBox_b5PEJH | box | field width, 4px under it, 1px --border-subtle, radius 8, --background-surface-higher, 0 2 8 black 16%, padding 4 | search_filters.rs option_list | ✅ |
| list height | screen | rows | 40px rows, five visible then a scroller | option_list / list_h | ✅ |
| De / Mentions rows | screen | content | 24px avatar, gap 8, display name 16/20, username on the right (muted), 20px checkbox; you first | options() users | ✅ (order after you: recent authors then members, the client's exact ranking is not known) |
| Dans rows | screen | content | channel glyph, name, checkbox; open channel first | options() "in:" | ✅ (the rest in sidebar order; the client's order looks frecency-based) |
| Contient rows | screen | content | glyph + image, vidéo, lien, fichier, intégration (more under the scroll) | HAS | ✅ first five; son / autocollant / sondage and the glyphs are ours |
| Type d'auteur / Épinglé | dump search-more-filters | kind | Type d'auteur is a combobox (input), Épinglé a plain select (`selectButton_`) | field(), options() | ✅ Type d'auteur combobox (Utilisateur / Bot / Webhook) + Épinglé select (Vrai / Faux) wired with option lists |
| open field | screen | style | --input-border-active border, chevron up | field() | ✅ |
| autocomplete | comboBoxInput_vWC4bk | behaviour | typing filters the list (exact, prefix, contains) | options() score | ✅ |
| picked values | — | style | pills before the text | field() | ✅ removable pills (`×`) rendered before the text input |
| popout | CDP | behaviour | "Plus de filtres" hides the search popout (bar back to idle); closing the modal (Escape) brings the popout back with the bar focused | search.rs render | ✅ |
| calendar / type list / option list | — | position | flipped above the field when they would pass the window bottom | render | ✅ |

⚠ During this pass the client sent one `POST /channels/<id>/messages/<id>/ack` (new message
in the open, focused channel while the modal was up). Following the brief the modal was closed
and not reopened, so Type d'auteur / Épinglé were not measured open.

Implementation note: popovers inside a `modal::frame` must not use a nested `deferred` (it paints
under the card's footer); search_filters.rs places them beside the card from canvas probes.

# Search results panel (live CDP 2026-11-24, 1536×840 @1.25, Onyx)

Searches run on the client ("merci", page 2, an empty query `zqxjvkwq`), each cleared afterwards
with the bar's clear button (route and bar text checked unchanged). No non-GET request went out.

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| sizes | compare 1536×788 and 1280×720 @1.25 (client emulated in DIP: w×1.25, dsf 1, its page zoom is 125%) | — | header, groups, cards and pager line up | — | ✅ |
| panel | searchResultsWrap_ | box | 419 wide under the header, black in Onyx (all base tokens are black there) | search.rs results_panel: `.searchResultsWrap_QHVbp3{background:var(--background-gradient-lowest, var(--background-base-lowest)); border-inline-start:1px solid var(--app-frame-border); width:418px}` → `--background-base-lowest`, 1px `--app-frame-border` left (was base-lower / border-subtle) | ✅ from the CSS |
| header | searchHeader_ | layout | 48px, padding 8 16; total body-md/medium --text-strong ("4 083 résultats", "Pas de résultat") | results_panel, group_digits | ✅ |
| header buttons | button_ sm secondary hasText | style | 32px, radius 8, 1px border, --background-mod-subtle, padding 3 11, 16px glyph + 14/18 medium, gap 8: "Filtres" (opens the modal), "Trier" | sm_secondary | ✅ |
| sort menu | menu_ flexible_ | box / rows | 144 wide under "Trier", right-aligned, 1px --border-subtle, radius 8; 36px radio rows (padding 8): Plus récents / Date / Pertinence | sort_popover, SortOrder → `sort_by=timestamp|relevance` | ✅ |
| scroller | scroller_ thin | padding | 16 16 0 | results_panel | ✅ |
| channel group | searchResultGroup_ / channelNameContainer_ | layout | group margin-bottom 24; 22px header (16px glyph + padding 4, name 16/20 medium --text-strong, margin-bottom 8); right: folder glyph + category 12/16 medium --text-default | group_header | ✅ |
| hit card | searchResult_ | box | 1px --border-subtle, radius 8, margin-bottom 8; cozy message: 40px avatar at 20, text at 80, name 16/22 medium + tag / role icon / APP, time 12 medium --text-muted ("14:58", "Hier à 13:23", "19/11/2026 18:52") | result_card | ✅ |
| match highlight | span.highlight | colour / words | `.highlight` hsl(--yellow-300 / .3) = rgba(253, 184, 51, .3). Words: the client's highlighter (js 698890330839a6c6 module 697530): query split on `/\W+/` (non-ASCII letters split words: "règlement" lights "r" + "glement", seen live), possessive `'s` dropped, lowercase, English stop words out, Porter2 stems (module 83956) compared token by token; applied to every markdown node with string content but code blocks (bold, italic, underline, strike, spoiler, links, inline code, headings, lists, quotes, and user / role / channel mentions, whose parse rules carry a `{type: text, content: "@name"}` child); not emoji, custom emoji, timestamps or sounds (no string content); the group's channel name too (`channelNameText_`, seen live); never inside Components V2 text (live: no `.highlight` under `isComponentsV2_`) | search_highlight.rs, richtext::apply_highlights, search.rs highlight_plain | ✅ |
| reply | repliedMessage_ messageSpine_ | layout | spine + 16px avatar + name + preview, or the italic "Le message n'a pas pu être chargé" placeholder | reply_line | ✅ |
| components v2 | container isComponentsV2_ | content | bot cards drawn like the chat, interactions disabled (`shouldDisableInteractiveComponents`, live: `cursor: not-allowed` inside the buttons) | components::render_static | ✅ (live search dumps: sections + green/red buttons, file, gallery, primary button with animated emoji) |
| header bar | searchBar_ | border | idle border while results show, active while typing | live_bar | ✅ |
| Jump | buttonsContainer_ > button_ | hover | "Y aller", 24px, padding 4, radius 3, 12/16 medium, top-right on hover; jumps to the message | result_card (jump_to_message) | ✅ |
| pager | paginationDock_ / pageControl_ | layout | 56px dock, padding 0 16; 40px "Précédent" / "Suivant" (caret 16, 16/18 semibold, radius 8); 28px round pages 8px apart, current --brand-500 + white; "1 2 3 … 164" | pager, page_items | ✅ |
| empty | emptyResultsWrap_ | layout | 280px column centred, 160px illustration, text 16/24 medium --text-default | empty_state | ✅ layout (our own illustration and wording) |
| error | — | — | not reproducible on the client with a GET | empty_state(error) | ➖ the client's error state only shows when the search request fails; making it fail on the live account would mean breaking its network or sending malformed requests |

# DM search (live CDP 2026-11-24, 1536×840 @1.25, Onyx, cross-DM setting off)

Searched inside the DM the user had open ("merci" → empty, "serveur" → 2 hits), gear menu
opened and closed with Escape, search cleared; route unchanged, no non-GET request but `/science`.
Client JS: `web.b63d846b329d6470.js` module 5990 (eligible tokens), 902008 (guild contexts),
`9329220d4bdc3583.js` (`SearchFetcher`: GET for GUILD / GUILD_CHANNEL / CHANNEL, POST tabs for DMS),
`fb74ed8e7dc14b59.js` (`createRequestPayload`, tab reply), module 768570 (tab defaults).

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| request | network | endpoint | `GET /channels/{id}/messages/search?content=…&sort_by=timestamp&sort_order=desc&offset=0` (the client always sends `offset`) | rest/search.rs `search_channel_messages` | ✅ (Phoenix omits `offset=0`) |
| placeholder | aria of the header combobox | text | "Rechercher staar" (`E3`: username of the 1:1 peer, group name for a group DM); "Chercher dans les MP" (`m7OrlR`) with the cross-DM setting | chat.rs `search_ph` | ✅ |
| popout filters | screen + module 5990 | rows | de: / contient: / mentions: / Plus de filtres; `in:` only in guild contexts or cross-DM | search.rs options_popout | ✅ |
| "Chercher dans …" row | 95de45d742273fa4.js EMPTY mode (`VGEH/0`) | row | cross-DM only: magnifier + "Chercher dans <tag>", runs `in: <tag>` | search.rs options_popout | ✅ (not seen live: setting off) |
| results header | screen | buttons | Filtres, Trier, then a 32×32 icon-only "Paramètres" (gear) button in DM contexts only | search.rs icon_secondary | ✅ |
| gear menu | `menu_M3qk4j flexible_M3qk4j` | box / rows | 237 wide, right edge on the gear, 7px under it; "Par défaut…" (`/tMwrA`, muted) then radios "Rechercher dans ce MP" (`jRkYAh`) / "Rechercher dans tous les MP" (`lWpJ/t`) | search.rs dm_popover | ✅ |
| gear menu action | `transitionStateToSearchContext` | behaviour | saves `text_and_images.is_cross_dm_search_enabled` (6, 36, BoolValue, default false) and reruns the query in the new context (`in:` dropped when going back to one DM) | set_cross_dm (settings `dm-search`) | ✅ (never clicked on the account) |
| group header | `channelNameIcon_3Zk3yP` + `channelNameText_3Zk3yP` | content | 16px AtIcon (padding-end 4) + the peer's display name, body-md/medium --text-strong | search.rs channel_group_header | ✅ 1:1 and group DMs: `getChannelIconComponent` (module 713654 `gU`) returns `AtIcon` for both `DM` and `GROUP_DM` (was a members glyph for group DMs) |
| all DMs | `SearchFetcher.DX` + `createRequestPayload` | request | `POST /users/@me/messages/search/tabs` `{include_nsfw, channel_ids, tabs: {messages: {sort_by, sort_order, …filters, offset, limit: 25}}, track_exact_total_hits: true}`; reply `tabs.messages.{total_results, messages}` | rest/search.rs `to_tabs_body`, `search_dms_tabs`, `parse_tabs_reply` | ✅ unit tests; not run on the account (setting off, POST) |
| global DM search | Friends page | — | the 2026 client has no separate "search all DMs" field: it is the DM header search with the setting on | — | ✅ (nothing to add) |

## Super Reaction toggle (2026-11-28)

| element | source | value | our code | status |
|---|---|---|---|---|
| header toggle | b1b275d45cc7cf27.js `t7` (rendered after the search bar only for `EmojiIntention.REACTION`), CSS 748798 `label_pfW0SY` / `labelChecked_pfW0SY` | 20px SuperReactionIcon, padding 6, --radius-lg, --background-base-lowest + --text-default; checked --brand-500 with a white icon; tooltip « Activer les super réactions » (`buV4av`) / « Super réactions activées » (`5cRA/b`) | ui/pickers/grid.rs burst_toggle | ✅ visual only: the pick still sends a normal reaction (super reactions spend Nitro credits and are a protected write); ✅ 360° rotation on each toggle (react-spring `t9` tension 750 / mass 2.5 / friction 70, off with reduced motion); ✅ first-use education for Nitro users (`SUPER_REACTION_TOGGLE_EDUCATION_DESKTOP`, forced rich tooltip 200 ms after mount: NitroWheelIcon 20, title « Super réactions illimitées ! » `ORK94p`, body `Osi/uy`, closed by a click). The dismissal is kept locally (`prefs.local.ui`), never sent: the client's dismiss is a settings-proto write |
| burst picker ring | CSS 381575 `isBurstReactionPicker_-sZhZn` | box-shadow 0 0 0 2px --background-brand, plus a 100px --background-brand glow at .5 | ui/pickers/mod.rs render_picker | ✅ |
| burst search placeholder | `defaultSearchPlaceholder: wT(intention, isBurst)` | not located in the cached bundle | — | ❌ |

