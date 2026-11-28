# Live comparison, client vs Phoenix (real account)

Scores from `scripts/compare/compare.py` (`mismatch_pct`, pixels whose difference is above 24/255), client window 1154×835 at devicePixelRatio 1.25, same route, Phoenix rendered offscreen by `real_shots`. Images stay private on /mnt/windata.

## Earlier real-data sweeps (24–25/11, `real_shots` vs the client)

Scores only: the captures show the user's real account and stay in
`/mnt/windata/project-phoenix/compare/sweep/` (never in git).

## Method (2026-11-24, wt/sweep)

- Client: the live 2026 client over CDP, Onyx, cozy, font 16, page zoom 125 %. Each view is
  one short CDP pass (2–9 s) under `flock /tmp/discord-cdp-driver.lock`:
  `Emulation.setDeviceMetricsOverride` to W×H **CSS px** at dpr 1.5 (width/height × zoom,
  device scale 1.5 / zoom, because the emulated size is in DIP), open the view, read the region
  boxes and the state `real_shots` needs (scroll anchors, sidebar width, audio, folders),
  screenshot, undo our own actions, `Emulation.clearDeviceMetricsOverride`.
  `Network.enable` logs every non-GET request of the pass.
- Phoenix: int-bin `real_shots` at the same W×H, `PHX_SCALE=1.5`, rendered after the pass
  from the saved state (`PHX_SCROLL`, `PHX_SIDEBAR_W`, …) so the client is not held.
  `PHX_INBOX_COLLAPSE=<n>` collapses the n-th Unreads card (with `PHX_VIEW=inbox PHX_TAB=unreads`).
  `PHX_JUMP=<message id>` jumps to an older message (`?around=` page, read only) before the shot.
- Score: `scripts/compare/compare.py --discord … --phoenix … --regions boxes.json`,
  `mismatch_pct` = share of pixels whose difference exceeds 24/255, per region.
- Skipped: a pass is skipped when the route changed since the previous pass (the user is
  using Discord: 5 min pause), when the user is typing, or when a modal of someone else is
  open. Views that did not open as expected are discarded.

Caveats: the two sides are a few seconds apart, so new messages, typing indicators, live
timers and presence changes count as mismatch. The messages region also counts scroll
offsets (when the client sits above the history Phoenix has loaded). Rows are for the route
the client showed at that time (guild A = 18-member text channel, guild B = 44-member text
channel).

Views not measured:
- Home / DMs: the client was never on `/channels/@me`, and the sweep does not navigate. Compared offline instead (wt/mute): `real_shots PHX_DM=<the dumped DM>` 1280×700 at dpr 1.5 against `dumps/vanilla-dm.png` (2026-11-23): title bar 2.7 %, rail 10.6 %, channel list 19.9 %, header 3.3 %, messages 42.6 %, composer 1.0 %, overall 28.6 %. The rail, list and messages differ by content (one day apart: new messages, DM order, NOUVEAU pill); layout and header line up. The dump's DM profile panel shows the full-art (hovered / animated theme) state, which a still dump cannot score: ➖ live measure only when the user opens a DM; the agents never navigate there (it would ack the DM).
- Profile popout: dropped after one pass, because an `ack` POST left the client while the
  popout was open. It was the client's own read ack for a message that arrived in the open
  channel, but the brief says to close such a view and not reopen it.
- Search at 1536×788: discarded (the search did not open, another agent's modal was on top).

## Before (int-bin 2f62f74)

Mismatch % per region.

| Size | View | Title bar | Rail | Channels | Header | Messages | Composer | Members | User panel | Inbox | Search | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1280×720 | channel (B) | 1.8 | 4.6 | 3.1 | 4.2 | 26.0 | 4.8 | 19.0 | 8.0 | — | — | **14.6** |
| 1280×720 | inbox (B) | 1.9 | 4.6 | 3.1 | 4.6 | 15.3 | 4.8 | 16.5 | 8.0 | 9.4 | — | **10.3** |
| 1280×720 | search (B) | 1.8 | 4.6 | 3.1 | 4.1 | 28.6 | 4.5 | — | 8.0 | — | 15.3 | **17.4** |
| 1536×788 | channel (A) | 2.5 | 30.7 | 2.9 | 3.6 | 8.0 | 2.5 | 7.5 | 8.2 | — | — | **7.2** |
| 1536×788 | inbox (B) | 1.5 | 4.6 | 3.0 | 3.3 | 12.3 | 5.9 | — | 8.2 | 9.4 | 15.0 | **9.0** |
| 1920×1080 | channel (B) | 1.2 | 31.6 | 3.2 | 2.4 | 4.3 | 2.3 | 17.9 | 8.1 | — | — | **6.7** |
| 1920×1080 | inbox (B) | 1.2 | 31.6 | 3.2 | 2.7 | 7.8 | 2.3 | 16.6 | 8.1 | 11.6 | — | **8.5** |
| 1920×1080 | search (B) | 1.2 | 31.6 | 3.2 | 2.1 | 9.5 | 2.8 | — | 8.1 | — | 23.9 | **9.1** |

## After (int-bin 478aa7f, same method, 15:16–15:20)

All rows on guild B (the client stayed there). Search at 1536×788 measured this time.

| Size | View | Title bar | Rail | Channels | Header | Messages | Composer | Members | User panel | Inbox | Search | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1280×720 | channel | 1.8 | 4.4 | 3.0 | 4.3 | 30.6 | 4.7 | 16.3 | 7.9 | — | — | **15.8** |
| 1280×720 | inbox | 1.9 | 4.4 | 3.0 | 4.6 | 10.4 | 4.8 | 14.2 | 7.9 | 11.1 | — | **8.0** |
| 1280×720 | search | 1.8 | 4.4 | 3.0 | 3.8 | 29.4 | 4.5 | — | 7.9 | — | 15.2 | **17.5** |
| 1536×788 | channel | 1.5 | 4.2 | 3.1 | 3.2 | 4.6 | 3.3 | 15.5 | 8.0 | — | — | **5.7** |
| 1536×788 | inbox | 1.5 | 4.2 | 3.1 | 3.6 | 10.8 | 3.4 | 13.6 | 8.0 | 11.7 | — | **8.2** |
| 1536×788 | search | 1.5 | 4.2 | 3.1 | 2.9 | 21.4 | 4.3 | — | 8.0 | — | 15.3 | **14.2** |
| 1920×1080 | channel | 1.2 | 3.8 | 3.2 | 2.4 | 8.4 | 2.3 | 15.0 | 7.9 | — | — | **7.5** |
| 1920×1080 | inbox | 1.2 | 3.8 | 3.2 | 2.7 | 12.6 | 2.3 | 13.5 | 7.9 | 13.3 | — | **9.6** |
| 1920×1080 | search | 1.2 | 3.8 | 3.2 | 2.1 | 12.9 | 2.8 | — | 7.9 | — | 23.9 | **10.3** |

Reading: the rail drops from 30–32 % to 4 % wherever the client rail was scrolled. The user
panel now shows the custom status (the 8 % left is text antialiasing and the ellipsis). The
messages region is decided by timing (new messages between the two captures) and by the
client sitting above the loaded history at 1280×720 (30 %). Member list scores are left to
wt/members2. Search results (15–24 %) are being rebuilt on wt/search.

## Gaps found, by visibility

| Gap | Cause (observed) | State |
|---|---|---|
| Server rail 30 % off whenever the client rail is scrolled | `real_shots` did not copy the rail `scrollTop` (91 px measured) | ✅ compare tool fixed (rail field in `SCROLL_JS`) |
| Member list "Activité — 6/7" vs "Activité — 1" / none | Phoenix counted feed entries by non-members as pending; see `responsive.md` | ✅ |
| Activity card "il y a 11min" vs green "12:08" timer | live entry (trait 3 `is_live`) times from `started_at` | ✅ |
| User panel "Ne pas déranger" vs the custom status; own member row without it | the member list sync sends our own row with `activities: []` (checked on the gateway: READY and SESSIONS_REPLACE carry the custom status), and that reset the own activities | ✅ |
| Header bell: the client shows the slashed bell on guild A's channel, Phoenix the plain bell | Client chat header `nH`: `BellSlashIcon` when `isChannelMuted \|\| resolvedMessageNotifications(channel) !== ALL_MESSAGES` (level: channel override, then category, then the server entry, then the server `default_message_notifications`). Guild A resolves to Only Mentions. `notifications::NotifyPrefs::header_bell_slashed` (wt/mute). Mute chain `isGuildOrCategoryOrChannelMuted` with `mute_config.end_time` (`tG`) is shared by notifications, guild unread, server icon and menus | ✅ wt/mute (read from READY only, no client menu opened) |
| Member list nameplate differs on one member (another image), nameplate brightness too high | Per-guild `member.collectibles.nameplate` was not parsed/overridden in `member_user`; `img_AFi54G` `mask-image: linear-gradient(to right, rgba(0,0,0,0.3) <fade>px, rgb(0,0,0) <fade+50>px)` attenuates nameplate art behind text while revealing right-side art at full `0.6` opacity on short rows (e.g. `Clem`: `151.5px -> 201.5px`, `bFant0m3`: `220px -> 270px`, measured live via CDP 2026-11-24). Dynamic fade mask + `bot_tag_px` double-margin removal (`wt/diff-members`, `957ff645`) dropped `member list` mismatch from 11.8 % (mean 3.9 %) to 10.8 % (mean 3.6 %) at 1536×840 dpr 1.25 | ✅ wt/sweep-audit + wt/diff-members |
| Nameplate "cut" at the right edge of member / DM rows: bright 2–4 dp strip on the last pixels, art washed out towards the right, texts of plated rows judged narrower | Row geometry already matched (live CDP 2026-11-24, 1536×840 @125 %: `childContainer_J1Adu_ nameplated_J1Adu_` 252 px, x 1276→1528, padding `0 32px 0 10px`; scroller `scrollbarGutterStable thin` 268 → content 260, so the 8 px gutter is reserved, not overlaid; `img_AFi54G` 223.925×41.99, `left 28.075px right 0`, `object-fit: fill`, `object-position: 100% 50%`, natural 448×84; one-line rows 213×40, DM 256×48). Real causes: (1) the baked mask clamped its end ratio to 1, so `fade` 220 on a 224 px image ramped .3→1 over the last 4 px instead of reaching .356 (client formula `linear-gradient(to right, rgba(0,0,0,.3) Fpx, #000 F+50px)` runs past the box); (2) the palette gradient was painted over the image, while it is `container_AFi54G`'s background, under the image (SBLOW right edge: client (36,10,38), before (29,12,44)); (3) `fade` was a text-length guess, the client reads `layout_` `contentRect.width + 10` (member list) / `- 5` (DM `layout_TBNqol`, ref `ti`) from a `ResizeObserver` (web.b63d846b329d6470.js module 351952). Phoenix now measures the same box with a probe (`nameplate::width_probe`) | ✅ wt/np-edge |
| Plated rows ellipsize one glyph earlier ("Happy 03…" vs "Happy 03/1…") | Same limit (1496 CSS px, `padding-right 32`), but GPUI's `truncate_line` trims trailing ASCII punctuation and whitespace before the ellipsis (`vendor/gpui-pre` line_wrapper.rs) and uses per-char advances; CSS `text-overflow: ellipsis` keeps them. Fixed by GPUI patch 4 `truncate_line_end_shaped` (docs/gpui-patches.md, text.md "Ellipsis": longest grapheme prefix + "…", punctuation kept; live 1280×720 identical) | ✅ |
| Member list one name ellipsized early (combining marks / emoji in styled name) | member list zone: cluster font fallback for combining marks, GPUI patches 5–7 (text.md "Combining marks": every glyph within 1 device px of the client capture, no early ellipsis) | ✅ |
| Search results panel: old layout (query field, "Plus récents / Plus anciens" tabs, plain author names, no highlight, no reply line, no embeds) vs the client's "N résultats" + Filtres + Trier header and full message cards | search results rebuilt by wt/search (MERGED 58273ab): "N résultats" header, Filtres / Trier, full message cards, pager (pickers.md search rows) | ✅ |
| Voice channel member rows: the client shows "Muet" + "En sourdine" for a self-muted, self-deafened member; one Phoenix render had the row without the icons, a later one had no voice rows at all | Three causes: `apply_ready` read the old voice users after `clear_session` had emptied `channels` (dead code, so the users from READY_SUPPLEMENTAL were lost when the ReadyData came after it); voice states from GUILD_CREATE or already cached at READY never sent their `VoiceFlags` (row without icons); my own row used the local mic buttons even when another client of mine held the call. Fixed, test `voice_users_survive_ready_after_supplemental`. Icons measured live (2026-11-24): `icon_YUoOxm` 16×16, gap 4 (`iconGroup_`), `--channels-default`, `iconServer_` `--icon-feedback-critical`, order Muet then En sourdine, no hover rule; glyphs = extracted MicrophoneSlashIcon / HeadphonesSlashIcon | ✅ wt/mute |
| Channel list "Nouveaux messages non lus" pill missing | compare artefact: with `NO_ACKS` the selected channel stays unread in `real_shots`, which hides the pill | — (not a product bug) |
| Message & reply usernames missing role color gradients (`twoColorGradient_` / `usernameGradient_`) | `chat.rs` used `emoji_text::label` for message authors and `fonts::name_text` for reply usernames instead of `role_gradient::for_author` + `fonts::name_text_with` (verified live via CDP, 2026-11-24) | ✅ wt/diff-messages (`957ff645`) |
| `article` link embeds rendered large preview image as 80×80 side thumbnail | `chat.rs` `embed()` only promoted `video` thumbnails to full-width media; Discord `sanitizeEmbed` also promotes `article` thumbnails (verified live on `immich.app` 1024×576 embed) | ✅ wt/diff-messages (`957ff645`) |
| Two-item media mosaic (`oneByTwoGrid_`) too short (~155px vs 273px), shifting all messages above by ~100px | `mosaic.rs` `Row::Two` used `row_aspect(its) * 2.` instead of 1:1 square cells (`ratio_box(2., 280.)`, measured live 550×273 / 273×273 per cell); `messages` mismatch dropped 31.1 % → 10.2 %, overall 17.1 % → 7.5 % | ✅ wt/diff-messages (`635719ae`) |
| Composer 5.4% → 2.3%, Header 3.5% → 2.4%, selected channel icons | Composer `channelTextArea_` border 0.8px vs GPUI 1px shifted the whole box 1px up (`mb` -0.8px, placeholder `top` -1.5px, buttons `mr` 13px); header search bar placeholder `pl` 8px / `lh` 18px and icon `--icon-muted`; selected channel action icons `--interactive-text-active`; thread row unread pill (`wt/diff-chrome-composer`, 957ff645) | ✅ wt/diff-chrome-composer |

## Sweep 2 (2026-11-25, wt/sweep2): popouts, settings, voice rows

Same method, extended driver `~/discord-phoenix-ref/sweep/{sweep2.mjs,run2.py,boxes2.js}`
(views `channel members phx phxmembers pins threads emoji gif sticker plus inbox search
set-<page>`; the popout box is the last `layer_` of `layerContainer_`, the settings boxes are
`aside.sidebar_cR4ic2` and the rest of the layer). Sizes 1536×840 and 1280×720, dpr 1.5,
client page zoom 125 %. `real_shots` now follows the client member list toggle
(`PHX_MEMBERS`), so views where the user hides the list are scored without it.

Notes on the passes:
- The client sends `POST /science` and `/metrics/v2` (telemetry) on every popout, and the read
  `ack` of the open channel (#général, or #phoenix when the pass visits it) when a message
  arrives during inbox / search. No other non-GET request was seen.
- The members pass first left the client member list open (the undo looked for the "Afficher"
  label, which becomes "Masquer la liste des membres" once open). It was closed by hand right
  after; the driver now clicks the "Masquer" label.
- The 2026 settings sidebar has no "Raccourcis clavier" entry, so there is no keybinds page to
  compare.
- Scores of the "after" table were rendered from the saved client state with int-bin 492a6177,
  after the GPUI baseline fix (5b4da8a5). The intermediate int-bin d9c87f70 drew every text
  about 5 px low (the channel list header was clipped), so its scores are not used.

### Before (1536×840 int-bin 20926677, 1280×720 int-bin 9889a054)

| Size | View | Title bar | Guild rail | Channel list | Header | Messages | Composer | Member list | User panel | Popout | Search results | Settings nav | Settings content | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1536×840 | channel | 1.5 | 2.9 | 3.9 | 2.5 | 12.5 | 10.8 | — | 7.2 | — | — | — | — | **9.3** |
| 1536×840 | members | 1.5 | 2.9 | 3.9 | 2.5 | 7.0 | 4.1 | 9.1 | 7.2 | — | — | — | — | **6.0** |
| 1536×840 | phx | 1.5 | 2.8 | 2.6 | 2.8 | 11.9 | 10.4 | — | 7.2 | — | — | — | — | **8.6** |
| 1536×840 | pins | 1.5 | 2.9 | 3.9 | 2.5 | 18.5 | 10.4 | — | 7.2 | 18.5 | — | — | — | **12.8** |
| 1536×840 | threads | 1.5 | 2.9 | 3.9 | 2.6 | 9.8 | 4.1 | 8.9 | 7.2 | 12.1 | — | — | — | **7.2** |
| 1536×840 | emoji | 1.5 | 2.9 | 3.9 | 2.5 | 23.9 | 3.8 | 8.8 | 7.2 | 42.1 | — | — | — | **13.6** |
| 1536×840 | gif | 1.5 | 2.9 | 3.9 | 2.5 | 14.9 | 3.7 | 8.8 | 7.2 | 20.1 | — | — | — | **9.5** |
| 1536×840 | sticker | 1.5 | 2.9 | 3.9 | 2.5 | 33.4 | 10.3 | — | 7.2 | 48.1 | — | — | — | **21.7** |
| 1536×840 | plus | 1.5 | 2.9 | 3.9 | 2.6 | 13.4 | 11.6 | — | 7.2 | 12.0 | — | — | — | **9.9** |
| 1536×840 | inbox | 1.6 | 2.9 | 3.9 | 3.2 | 18.9 | 10.8 | — | 7.2 | 29.5 | — | — | — | **13.1** |
| 1536×840 | search | 1.5 | 2.9 | 6.5 | 2.8 | 15.9 | 5.6 | — | 7.2 | — | 20.3 | — | — | **11.7** |
| 1536×840 | set-account | 0.6 | 0.3 | 2.2 | 0.1 | 1.7 | 2.8 | — | 2.0 | — | — | 2.8 | 1.6 | **1.7** |
| 1536×840 | set-privacy | 0.6 | 0.3 | 2.4 | 1.0 | 9.6 | 4.7 | — | 2.0 | — | — | 3.1 | 8.1 | **6.6** |
| 1536×840 | set-appearance | 0.6 | 0.3 | 11.5 | 0.1 | 6.7 | 9.5 | — | 28.1 | — | — | 15.9 | 6.2 | **7.1** |
| 1536×840 | set-voice | 0.6 | 0.3 | 12.1 | 0.2 | 4.9 | 3.6 | — | 2.7 | — | — | 16.7 | 4.3 | **5.8** |
| 1536×840 | set-notifications | 0.6 | 0.2 | 2.0 | 0.2 | 4.6 | 2.8 | — | 2.0 | — | — | 2.5 | 3.9 | **3.4** |
| 1280×720 | channel | 1.8 | 3.0 | 2.8 | 3.3 | 19.0 | 10.7 | — | 7.2 | — | — | — | — | **12.0** |
| 1280×720 | members | 1.8 | 3.0 | 2.6 | 3.1 | 13.0 | 4.4 | 9.7 | 7.2 | — | — | — | — | **7.9** |
| 1280×720 | phx | 2.7 | 3.3 | 7.6 | 5.6 | 6.5 | 5.0 | — | 8.8 | — | — | — | — | **6.3** |
| 1280×720 | pins | 1.8 | 3.0 | 2.6 | 3.1 | 12.2 | 4.4 | — | 7.2 | 17.9 | — | — | — | **7.9** |
| 1280×720 | threads | 1.8 | 3.0 | 2.6 | 3.2 | 6.0 | 3.3 | — | 7.2 | 9.8 | — | — | — | **4.5** |
| 1280×720 | emoji | 1.8 | 3.0 | 2.6 | 3.0 | 28.9 | 3.6 | — | 7.2 | 42.0 | — | — | — | **16.7** |
| 1280×720 | gif | 1.8 | 3.0 | 2.6 | 3.0 | 19.0 | 3.4 | — | 7.2 | 21.8 | — | — | — | **11.5** |
| 1280×720 | sticker | 1.8 | 3.0 | 2.6 | 3.0 | 30.6 | 3.4 | — | 7.2 | 45.2 | — | — | — | **17.6** |
| 1280×720 | plus | 1.8 | 3.0 | 3.7 | 3.0 | 8.9 | 4.6 | — | 7.2 | 15.0 | — | — | — | **6.4** |
| 1280×720 | inbox | 1.9 | 3.0 | 3.7 | 4.1 | 22.0 | 3.2 | — | 7.2 | 29.2 | — | — | — | **13.4** |
| 1280×720 | search | 1.8 | 3.0 | 3.7 | 3.2 | 18.9 | 4.8 | — | 7.2 | — | 18.1 | — | — | **11.9** |
| 1280×720 | set-account | 0.7 | 1.1 | 2.0 | 0.1 | 1.5 | 2.4 | — | 2.5 | — | — | 2.8 | 1.3 | **1.6** |
| 1280×720 | set-privacy | 1.9 | 1.5 | 5.2 | 1.4 | 11.2 | 8.0 | — | 4.4 | — | — | 7.1 | 9.2 | **8.1** |
| 1280×720 | set-appearance | 1.9 | 5.7 | 8.1 | 0.1 | 5.9 | 1.5 | — | 4.6 | — | — | 12.7 | 4.6 | **5.7** |
| 1280×720 | set-voice | 1.9 | 5.7 | 7.9 | 0.1 | 4.9 | 2.7 | — | 4.6 | — | — | 12.4 | 4.0 | **5.2** |
| 1280×720 | set-notifications | 1.9 | 5.9 | 10.5 | 0.1 | 5.9 | 6.0 | — | 5.1 | — | — | 16.2 | 5.0 | **6.6** |

### After

| Size | View | Title bar | Guild rail | Channel list | Header | Messages | Composer | Member list | User panel | Popout | Search results | Settings nav | Settings content | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1536×840 | channel | 1.5 | 2.8 | 3.4 | 2.3 | 4.3 | 3.2 | — | 7.5 | — | — | — | — | **3.8** |
| 1536×840 | members | 1.5 | 2.8 | 3.4 | 2.4 | 5.8 | 3.6 | 22.2 | 7.5 | — | — | — | — | **7.3** |
| 1536×840 | phx | 1.5 | 2.8 | 3.5 | 2.6 | 4.3 | 3.1 | — | 7.5 | — | — | — | — | **3.8** |
| 1536×840 | pins | 1.5 | 2.8 | 3.4 | 2.4 | 10.6 | 2.7 | — | 7.5 | 18.6 | — | — | — | **7.5** |
| 1536×840 | threads | 1.5 | 2.8 | 3.4 | 2.5 | 4.9 | 3.1 | — | 7.5 | 9.4 | — | — | — | **4.2** |
| 1536×840 | plus | 1.5 | 2.8 | 3.4 | 2.3 | 4.6 | 4.2 | — | 7.5 | 12.6 | — | — | — | **4.0** |
| 1536×840 | inbox | 1.6 | 2.8 | 3.4 | 3.3 | 11.8 | 3.2 | — | 7.5 | 19.0 | — | — | — | **8.3** |
| 1536×840 | search | 1.5 | 2.8 | 3.4 | 2.5 | 12.4 | 5.1 | — | 7.5 | — | 20.6 | — | — | **8.9** |
| 1536×840 | set-privacy | 0.6 | 0.2 | 2.4 | 1.0 | 9.3 | 2.4 | — | 2.0 | — | — | 3.1 | 7.7 | **6.3** |
| 1536×840 | set-appearance | 0.6 | 0.2 | 10.8 | 0.1 | 4.0 | 8.1 | — | 27.9 | — | — | 14.9 | 3.9 | **5.2** |
| 1536×840 | set-voice | 0.6 | 0.2 | 11.1 | 0.2 | 3.2 | 1.0 | — | 3.5 | — | — | 15.4 | 2.7 | **4.4** |
| 1280×720 | channel | 1.8 | 3.0 | 3.7 | 3.0 | 5.7 | 3.8 | — | 7.3 | — | — | — | — | **4.7** |
| 1280×720 | members | 1.8 | 3.0 | 3.7 | 3.2 | 8.8 | 3.9 | 19.7 | 7.3 | — | — | — | — | **8.4** |
| 1280×720 | phx | 1.8 | 3.0 | 3.8 | 3.5 | 6.1 | 3.8 | — | 7.3 | — | — | — | — | **4.9** |
| 1280×720 | pins | 1.8 | 3.0 | 3.7 | 3.1 | 11.3 | 3.9 | — | 7.3 | 17.8 | — | — | — | **7.6** |
| 1280×720 | threads | 1.8 | 3.0 | 3.7 | 3.2 | 4.9 | 2.8 | — | 7.3 | 9.8 | — | — | — | **4.2** |
| 1280×720 | plus | 1.8 | 3.0 | 3.7 | 3.0 | 6.3 | 4.6 | — | 7.3 | 12.6 | — | — | — | **5.0** |
| 1280×720 | inbox | 1.9 | 3.0 | 3.7 | 4.2 | 14.3 | 2.8 | — | 7.3 | 18.9 | — | — | — | **9.3** |
| 1280×720 | search | 1.8 | 3.0 | 3.7 | 3.3 | 14.9 | 4.2 | — | 7.3 | — | 18.3 | — | — | **9.8** |
| 1280×720 | set-privacy | 0.7 | 1.2 | 2.3 | 1.0 | 11.2 | 8.9 | — | 2.5 | — | — | 3.2 | 9.2 | **7.3** |
| 1280×720 | set-appearance | 0.7 | 5.6 | 7.6 | 0.1 | 5.7 | 1.0 | — | 2.6 | — | — | 12.3 | 4.3 | **5.4** |
| 1280×720 | set-voice | 0.7 | 5.5 | 7.3 | 0.1 | 4.4 | 1.6 | — | 2.6 | — | — | 11.8 | 3.5 | **4.7** |
| 1536×840 | emoji | 1.5 | 2.9 | 6.5 | 2.5 | 27.8 | 3.4 | 20.4 | 7.5 | 35.4 | — | — | — | **17.7** |
| 1536×840 | gif | 1.5 | 2.9 | 6.5 | 2.5 | 17.7 | 3.3 | 20.4 | 7.5 | 10.9 | — | — | — | **13.1** |
| 1536×840 | sticker | 1.5 | 2.9 | 6.5 | 2.3 | 21.2 | 2.9 | — | 7.5 | 33.2 | — | — | — | **14.6** |
| 1280×720 | emoji | 1.8 | 3.0 | 2.6 | 3.0 | 26.8 | 3.2 | — | 7.2 | 34.8 | — | — | — | **15.6** |
| 1280×720 | gif | 1.8 | 3.0 | 2.6 | 3.0 | 14.7 | 3.0 | — | 7.2 | 10.2 | — | — | — | **9.1** |
| 1280×720 | sticker | 1.8 | 3.0 | 2.6 | 3.0 | 26.0 | 3.1 | — | 7.2 | 33.2 | — | — | — | **15.2** |

emoji / gif / sticker rows of the after table reuse the client captures of the before pass (the client was restarted on Home before they could be taken again), so their message and member list regions still compare against the earlier client state; only the popout column is meaningful there. Member list rows drift with presence (the re-render is later than the capture).

### Gaps closed by wt/sweep2

| Gap (visibility) | Cause | State |
|---|---|---|
| Channel list "Nouveaux messages non lus" pill spans the whole list (medium) | 2026 pill `bar_t1P8Sn` is sized to its label and centered in `container_t1P8Sn` (flex, justify center, top 8): padding 4 8, radius 20, 0.8 px border, shadow 0 12 24 black 24 % | ✅ `channel_unread_bar.rs` |
| Voice user rows: plain name font, LIVE badge before the mute / deafen icons (medium) | the client uses the display name style font (`dnsFont_*`) on `usernameFont_YUoOxm` and puts `badgeWrapper_` after the icons in `iconGroup_YUoOxm` | ✅ `voice_panel.rs` |
| Member list: speaker icon for a member who streams in voice (small) | client draws `ScreenIcon` 12 px `#45a366` ("En partage d'écran") | ✅ `members.rs` |
| Pins / inbox / search cards: no avatar decoration (medium) | `cozy_message` and `result_card` drew the bare avatar | ✅ `inbox.rs`, `search.rs` |
| Threads popout 480 px instead of 537.6 at 1536 (medium) | `.browser_l-3BKx { width: 35vw; min-width 480px; max-width 600px }` | ✅ `inbox.rs` (+ `title_bar::viewport_w_css`) |
| Threads popout "Commencé par" name in normal weight, bright bullet (small) | `startedByName_yRVHqK` 600 + role color, `bullet_yRVHqK` rgba(151,151,159,.08), padding 0 8 | ✅ |
| "+" menu: upload and thread icons (small) | client `FileUpIcon`, `ThreadPlusIcon` (React fiber names) | ✅ `composer.rs`, `icons/names.rs` |
| Pin system message: arrow icon, no links (medium) | client `PinIcon` 18 px, "un message" and "messages épinglés" links in `--text-strong` 500 | ✅ `chat/system.rs` |
| Données et confidentialité: links missing in four descriptions (small) | "désactivant" / "supprimant" and "En savoir plus" are links in the live page | ✅ `settings/pages.rs`, `fr.yml` |
| Sticker picker footer "de Fréquemment utilisés" (small) | the footer names the pack or server ("de Wumpus Beyond"), never the section | ✅ `pickers/sticker.rs` |
| Expression picker 18 px too far right (medium) | live `contentWrapper_58b0l2` right edge 1502 at 1536 wide: 34 px from the chat edge, bottom 75.6 | ✅ `pickers/mod.rs` |

### Gaps still open, by visibility (for other agents)

| Gap | Probable cause | Where |
|---|---|---|
| ✅ (wt/invites) — Invite links (`discord.gg/…`) render as plain links: no "Tu as été invité(e)" card, no invalid-invite card, in chat and in pins | no invite embed at all (needs `GET /invites/{code}?with_counts=true` and the card) | `ui/chat.rs` embeds, `ui/inbox.rs` `cozy_message` |
| ✅ (wt/invites) — Emoji picker: no "Émojis populaires sur <serveur>" section (trophy rail icon, first row) and no "POPULAIRES" badge in the footer | client section `TOP_GUILD_EMOJI` (`TrophyIcon`, `topEmojiSectionContainer_6OcK5D`), data from the guild top-emojis store | `ui/pickers/` |
| ✅ (wt/unreads) Inbox "Messages non lus" — the list is built (wt/popouts) and loads on the real account; cards were blank because embeds were not drawn, the header lacked the check-double icon (icon extractor picked `ChatCheckIcon` from an earlier file), and the card bell / collapse buttons were missing | see popouts.md "Inbox > Unreads" | `ui/inbox.rs`, `icons/mod.rs` |
| ✅ (wt/details2) **Medium** — "+" menu 30–40 px lower than the client | client bottom = plus button top − 8; Phoenix bottom lands ~30 px under it (ATTACH_BTN bounds or anchored offset) | `ui/chat/composer.rs` `attach_menu` |
| **Medium** — "+" menu "Programmer un message" disabled, no chevron submenu | scheduled messages not implemented (client: `CalendarPlusIcon` + `ChevronSmallRightIcon`) | `composer.rs` |
| ✅ (wt/unreads) Threads popout "Commencé par" empty for an owner outside the member cache | `parity()` now sends archived thread / forum post owners and first authors to `ensure_authors` (op 8 for the missing ones) | `discord_backend.rs` |
| ✅ (wt/unreads) Search results: the name was ellipsized instead of wrapping the timestamp | the client header flows inline, so the time goes under a long name (ml 4); Phoenix header is now `flex-wrap` with no truncation. Server avatars: results already go through `ensure_message_authors`; the after capture shows them | `ui/search.rs` `result_card` |
| ✅ (wt/invites) — Frequent emoji / sticker order differs (✅ ❤ … vs ❤ …, Wave first in the client) | frecency scoring | `ui/pickers/` |
| ✅ (wt/unreads) Channel list showed the top pill where the client shows the bottom one | the rule (`ChannelListUnreadsStore` `rh`) was already the client's; rows below the rendered range of the virtual list had no bounds and were skipped, so the bottom side looked empty. Unmeasured rows after the scroll top now count as below. Still open: the client's "N in voice" bottom bar (`voice-channels` mode) | `channel_unread_bar.rs` |
| ✅ (wt/details2) **Medium** — Settings: the selected page does not scroll into view in the sidebar, Voix & Vidéo device names "Par défaut" vs the real names, modal ~3 px lower at 1280×720 | sidebar scroll, audio device enumeration | `ui/settings/` |
| ✅ (wt/details2) **Small** — Voice row avatars sharper than the client | client loads `size=32` for a 24 px avatar at dpr 1.5 | `ui::avatar` size choice |
| ✅ (wt/details2) **Small** — GIF picker "Favoris" tile shows another favorite | client uses the most recent favorite | `ui/pickers/` |
| ✅ (wt/details2) **Small** — Threads search field: client grows it (`container_VhFsFg` flex-grow 1), Phoenix 248 px | | `ui/inbox.rs` |
| — Voice message "0:02" vs "0:05" | client playback state (it had been played), not a product gap | — |

### Gaps closed by wt/details2 (measured live through CDP, 2026-11-25)

| Gap | Client reference | Fix |
|---|---|---|
| "+" menu 32 px too low | `menu_M3qk4j` layer `bottom: 62.8px; left: 441px` at 1536×840: menu bottom 777.2 = "Plus d'options de message" button top 785.2 − 8, left = button left. Same rule at 1280×720 | the `ATTACH_BTN` probe canvas was `absolute()` without insets inside a block container, so Taffy put it at its static position under the 32 px button. `composer.rs` adds `top_0().left_0()` |
| Expression pickers (emoji / GIF / stickers) anchor | layer `bottom: 75.6px; right: 10px`, `contentWrapper_58b0l2` 498×486, right edge 1502 at 1536 (unchanged since wt/sweep2) | ✅ already right; the Applications launcher was not opened (it writes `settings-proto/1`) |
| Settings sidebar does not reveal the selected page | `scrollSidebarNodeIntoView(node, {block: "nearest"})` with `padding: 12` on every settings navigation (web.js, settings scroll manager) | `settings/view.rs` `reveal_probe`: nearest scroll with a 12 px margin when the page changes; the sidebar starts at the top when settings open |
| Voix & Vidéo device select shows "Par défaut" | the default entry is the native default name with "Default" replaced by `Q3YKwS` ("Paramètres audio système par défaut"), e.g. "Paramètres audio système par défaut: Logitech G PRO X Gaming Headset Mono"; `deviceLabel_bUNrgk` 16/20 weight 500, `white-space: nowrap; overflow: hidden; text-overflow: clip` | `settings/voice.rs`: default label = `voice.system_default` + ": " + PipeWire/PulseAudio default (`pactl get-default-source/sink`, first in the list), weight 500, clipped. Read-only: Phoenix never changes the system default |
| Settings modal 3 px low at 1280×720 | modal frame 40→680 / 40→1240 in both captures with int-bin 492a6177 | no change needed (the gap came from the d9c87f70 baseline bug) |
| Voice row avatars sharper | voice user avatars are loaded with `?size=32` for a 24 px avatar | `ui::avatar_cdn` + `media::with_cdn_size`, used by `voice_panel.rs` |
| GIF "Favoris" tile shows another favorite | `favoritesTile: src = _.sample(_.values(favorites)).src` (random favorite, picked when the GIF picker mounts) | `pickers/gif.rs`: random favorite per picker opening; `real_shots` can pin the client's pick with `PHX_GIF_FAV` (URL substring) |
| Threads search field 248 px | `container_VhFsFg` flex 1 1 auto (content basis 205.2) and `spacer_c0tu5x` flex 1 share the free space: 276.5 px at 1536 (popout 537.6), 247.7 px at 1280 (popout 480) | `inbox.rs`: `flex_grow` + `flex_basis(205.2)` next to the existing `flex_1` spacer |
| Device names with accents showed "(null)" | `pactl -f json` cannot encode non-ASCII descriptions ("Stéréo") and prints `(null)` | `phoenix-voice` `pulse::devices` fills those from `LC_ALL=C pactl list sinks/sources` (UTF-8 text) |
| Device label runs into the chevron | `deviceLabel_bUNrgk` ends 14.3 px before the 20 px chevron (label 284.7 of a 340 select, padding 8 8 8 12) | label `mr(14.3)` |

Check pass after the merge (`compare/details2`, driver `sweep/{sweep3.mjs,run3.py}`, views prefixed `phx-` run in #phoenix): popout mismatch "+" 12.6 → 11.1 % at 1536 and 12.6 → 11.2 % at 1280 (menu bottom now 1 px from the client; the rest is text antialiasing), GIF 10.2 → 6.0 % at 1280 with the client's favorite pinned (`PHX_GIF_FAV`). Remaining: the threads popout is ~2 px right and 3–4 px high (already in the sweep 2 capture), and the sidebar scroll of the `set-*` captures is the driver's `scrollIntoView({block:'center'})`, not the client's own nearest scroll. The output select still differs because the client has an explicit output device chosen (Phoenix's own prefs use the system default), and the client focuses the GIF search field on open (2 px brand border) while Phoenix draws the idle border. A real_shots panic on every settings page (two `hover` styles on the footer More link, eade0fe7) was fixed on the way (f251b4c5).

Closed by wt/invites (2026-11-25): invite cards (profile card v2, invalid, legacy, group DM,
friend) in chat, pins, inbox and search; the top server emoji row with its trophy and POPULAIRES
badge; the frequent emoji and sticker order (client frecency, checked item for item on the live
proto). Details and what stays open (event, stream and voice v2 invite cards, the newly added
emoji block, reaction frecency) are in `message-content.md` (Invite cards) and `pickers.md`.

### wt/unreads (2026-11-25 07:15, int-bin 48c1fe8d)

Inbox "Messages non lus" at 1280×720: popout region 7.0 % mismatch / 2.8 % mean diff (after table above: 18.9 %; before: 29.2 %). The client was on `/channels/@me` during the pass, so the channel list and message regions of that run are not comparable. The 1536×840 client capture was discarded by the driver (view not in the expected state). Same 9 cards in the same order as the client; remaining differences: a user mention of a non-member in an embed shows "@Utilisateur inconnu" where the client shows the username it already knows (the client resolves it from its user cache), and the collapsed state is not persisted (`collapsedInInbox` in the settings proto).

## wt/fin-chat (2026-11-25)

- Threads popout: client box (details2 boxes.json) x 606–1144, y 81 at 1536×840; Phoenix was at right 382 / top 78 (10 px right, 3 px high, same at 1280). Now right `hdr_right(3) + 10` = 392, top = title bar + header. Names in the rows use the role gradient like the client (Yohan pink→orange, Boblebricoleur #8799ae→#f1f7ff, Phoenix drew the primary colour only), and the last message text no longer grows (`flex 0 1 auto`), so "• Il y a 8 h" follows it as in the client.
- GIF search focus: see pickers.md "search focused" ✅.
- Inbox embed mention of a non-member (`@Utilisateur inconnu` vs the username): ➖ same rule in both. The client's mention rule (`mention.parse`, web bundle) only reads `UserStore` and renders `@Utilisateur inconnu` (`sKdZ6U`) otherwise. The message (#bienvenue 1575048794351870063) is Components V2 with an empty `mentions` array and the user has left the server (GET member → 10007); the client knew them from its live session (member list while they were a member). Phoenix applies the same lookup on its own session cache.
- "N dans un salon vocal" channel list bar (`ChannelListUnreadsStore` mode `voice-channels`, text `fDlr+F`, VoiceNormalIcon 14 + 4 avatars 16): ➖ only exists when the guild channel list has the 2026 voice channels section (`voiceChannelsSectionNumber`); no such section in the user's client (DOM check 2026-11-25), so Phoenix has nothing to draw.
- ⚠ Navigating the official client to #phoenix from another route sent `POST /guilds/1369382919948210248/migrate-command-scope` (known, see the brief).

## Sweep 3 (2026-11-25, wt/sweep3): full pass after v0.1.0-beta.1

Driver `~/discord-phoenix-ref/sweep/{sweep4.mjs,run4.py,stable.sh,table3.py}` (sweep3.mjs plus the
2026-11-25 CDP rules): every CDP call has a 5 s limit (`Runtime.evaluate` `timeout` 4.5 s and a
race on the reply; exit 12 = client hung, the plan stops), a 28 s watchdog, refusal when the route
is not the expected one, when the user types, when a modal / menu is open, or when the window is
not painted (`requestAnimationFrame` stalled 1.2 s); the transparent `scrim_` layer the client puts
up after arriving on a guild is clicked away first. Navigation is a separate `nav:<path>` pass, so a
batch enters Aperture once and leaves once (no `migrate-command-scope` POST was seen this time). A
route change that is not ours stops the whole plan and `stable.sh` waits for 5 minutes of stable
`/channels/@me` before the next batch. Settings, server settings and channel settings are closed
with Escape, then the layer's "Fermer" button if Escape did not close it (it did not once on
"Système"; the page stayed open until the driver was fixed). New views: inbox tabs by index
(`inbox-0..4` = Messages non lus, Mentions, Messages marqués, Rappels, Programmés, the last pass
restores tab 0), `set-*` for every 2026 sidebar entry, `srv-*`, `chanset-*`, `home`, `discovery`,
`quests`. Sizes 1536×840 and 1280×720, dpr 1.5, page zoom 125 %. Scores = `compare.py` mismatch %
(pixels off by more than 24/255). "Before" = int-bin 97c3b9a0 (views first rendered after lot 1:
dbacd367), "after" = int-bin cf929fbd.

Writes seen: `POST /science` and `/metrics/v2` (telemetry) on most passes, nothing else, except
⚠ one `POST /channels/:id/messages/:id/ack` when the `dm` view opened a DM (12:37): the DM had an
unread message that the driver's row check did not see (no badge, no "non lu" label on the row).
The `dm` view is disabled in the driver and was not opened again. Opening the quests page,
Discover and every user settings page wrote nothing.

Not measured: server and channel settings (the first batch used the old `aside` selectors and found
no item; the 2026 server and channel settings sidebars are `[class*=sidebarRegion_] [role=tab]`,
fixed in the driver), and the Aperture views at 1280×720: the user was using the client from 13:22
(Discover › Applications, then other servers), so those passes never started. The DM view is the
one capture taken before it was disabled. "Mon compte" is compared but its captures are not
published anywhere.

### Scores

| Size | View | Title | Rail | Channels | Header | Messages | Composer | Members | DM panel | User panel | Popout | Search | Set. nav | Set. content | Overall before → after |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1536×840 | channel | 1.5 | 3.1 → 7.1 | 2.9 → 5.9 | 2.4 | 8.3 → 24.6 | 3.6 | 28.6 → 13.5 | — | 7.3 | — | — | — | — | 9.4 → **15.2** |
| 1536×840 | members | 1.5 | 3.1 | 2.9 | 2.4 | 8.3 → 24.6 | 3.6 | 28.6 → 13.4 | — | 7.3 | — | — | — | — | 9.4 → **14.4** |
| 1536×840 | phx | 1.5 | 2.9 | 9.2 | 2.3 | 16.2 → 29.5 | 3.5 | 28.9 → 29.0 | — | 8.1 | — | — | — | — | 14.3 → **20.4** |
| 1536×840 | pins | 1.5 | 3.1 | 2.9 | 2.4 | 14.9 → 24.7 | 3.1 | 28.9 → 14.3 | — | 7.3 | 15.2 → 15.3 | — | — | — | 12.4 → **14.6** |
| 1536×840 | threads | 1.5 | 3.1 → 7.1 | 2.9 → 5.9 | 2.6 | 9.6 → 23.4 | 3.6 | 27.1 → 14.4 | — | 7.3 | 6.1 | — | — | — | 9.7 → **14.9** |
| 1536×840 | plus | 1.5 | 3.1 | 2.9 | 2.4 | 7.0 → 19.3 | 3.3 | 23.0 → 14.4 | — | 7.3 | 11.1 → 11.0 | — | — | — | 7.9 → **12.2** |
| 1536×840 | emoji | 1.5 | 3.1 | 2.9 | 2.4 | 18.4 → 26.2 | 3.4 | 24.3 → 14.3 | — | 7.3 | 28.0 → 28.2 | — | — | — | 13.3 → **15.3** |
| 1536×840 | gif | 1.5 | 3.1 | 2.9 | 2.4 | 11.7 → 19.5 | 3.3 | 22.9 → 14.3 | — | 7.3 | 12.0 | — | — | — | 10.0 → **12.2** |
| 1536×840 | sticker | 1.5 | 3.6 → 3.1 | 2.9 | 2.4 | 11.2 → 17.9 | 3.3 | 22.9 → 14.3 | — | 7.3 | 10.7 → 8.1 | — | — | — | 9.8 → **11.5** |
| 1536×840 | search | 1.5 | 7.4 → 3.3 | 5.9 → 2.9 | 2.5 | 12.4 → 24.8 | 5.1 | — | — | 7.3 | — | 16.1 | — | — | 9.7 → **16.3** |
| 1536×840 | profile | 1.5 | 3.1 → 7.1 | 2.9 → 5.9 | 2.4 | 16.6 → 29.9 | 5.8 → 6.1 | 29.1 → 14.6 | — | 7.6 → 7.3 | 30.6 → 29.8 | — | — | — | 13.4 → **18.0** |
| 1536×840 | home | 1.3 | 5.6 → 5.3 | 5.9 → 10.3 | 5.0 | 10.0 → 9.0 | 7.4 → 9.8 | — | — | 7.6 | — | — | — | — | 8.3 → **8.7** |
| 1536×840 | dm | 2.1 | 5.3 | 6.0 → 18.9 | 3.3 | 5.2 | 3.5 | — | 7.0 | 7.4 → 7.5 | — | — | — | — | 5.4 → **8.2** |
| 1536×840 | discovery | 1.5 | 5.1 → 5.2 | 1.5 | 7.6 | 14.8 | 31.6 → 38.6 | — | — | 7.2 | — | — | — | — | 11.7 → **12.1** |
| 1536×840 | quests | 1.4 | 5.6 → 5.3 | 5.9 → 10.2 | 6.1 → 5.3 | 46.5 | 21.0 | — | — | 7.6 | — | — | — | — | 31.0 → **31.9** |
| 1536×840 | inbox-0 | 2.4 → 1.3 | 8.4 → 5.3 | 31.5 → 10.6 | 10.8 → 5.6 | 12.0 → 8.6 | 11.5 → 9.1 | — | — | 7.6 | 8.7 → 9.9 | — | — | — | 15.5 → **8.5** |
| 1536×840 | inbox-1 | 2.4 → 1.3 | 8.4 → 5.3 | 31.5 → 10.3 | 10.9 → 5.7 | 15.6 → 14.3 | 10.9 → 9.1 | — | — | 7.6 | 17.5 → 21.2 | — | — | — | 17.6 → **11.9** |
| 1536×840 | inbox-2 | 2.4 → 1.3 | 8.4 → 36.2 | 31.5 → 10.3 | 10.7 → 5.5 | 11.1 → 7.1 | 11.6 → 8.4 | — | — | 7.6 → 7.7 | 6.6 | — | — | — | 14.9 → **9.0** |
| 1536×840 | inbox-3 | 2.4 → 1.3 | 8.4 → 36.2 | 31.5 → 10.3 | 10.7 → 5.5 | 10.9 → 7.0 | 11.6 → 8.4 | — | — | 7.6 → 7.7 | 6.2 | — | — | — | 14.8 → **8.9** |
| 1536×840 | inbox-4 | 2.4 → 1.3 | 8.4 → 36.2 | 31.5 → 10.3 | 10.7 → 5.5 | 10.5 → 6.6 | 11.6 → 8.4 | — | — | 7.6 → 7.7 | 5.3 | — | — | — | 14.6 → **8.7** |
| 1536×840 | set-account | 0.7 | 2.0 → 15.1 | 2.2 | 0.2 | 1.6 → 1.5 | 1.4 → 2.6 | — | — | 2.1 | — | — | 2.8 | 1.4 → 1.5 | 1.7 → **2.3** |
| 1536×840 | set-privacy | 0.7 | 2.0 → 15.1 | 2.4 | 1.1 | 9.5 → 9.4 | 4.2 → 4.5 | — | — | 2.1 | — | — | 3.1 | 8.1 → 8.0 | 6.7 → **7.2** |
| 1536×840 | set-msgperms | 0.7 | 2.0 → 1.5 | 2.2 | 1.2 | 6.5 → 6.4 | 5.3 → 6.5 | — | — | 2.4 → 2.3 | — | — | 2.7 → 2.8 | 5.7 | 4.9 → **4.9** |
| 1536×840 | set-notifications | 0.7 | 2.0 → 1.5 | 2.0 | 0.3 | 4.4 → 4.3 | 1.8 → 2.3 | — | — | 2.1 → 2.0 | — | — | 2.5 | 3.8 → 3.7 | 3.3 → **3.3** |
| 1536×840 | set-voice | 0.7 | 2.0 → 1.5 | 10.8 | 0.3 | 3.5 | 2.4 → 2.8 | — | — | 2.7 → 2.6 | — | — | 15.0 | 3.0 | 4.7 → **4.6** |
| 1536×840 | set-appearance | 0.7 | 2.0 → 1.5 | 11.3 | 0.2 | 4.3 → 4.2 | 8.8 → 10.5 | — | — | 16.6 → 16.5 | — | — | 15.7 | 4.2 → 4.3 | 5.6 → **5.7** |
| 1536×840 | set-accessibility | 0.7 | 1.5 | 11.7 | 0.3 | 4.5 | 3.0 | — | — | 16.5 | — | — | 16.3 | 3.9 | — → **5.5** |
| 1536×840 | set-system | 0.7 | 2.0 → 1.5 | 11.8 | 0.2 | 5.6 → 5.5 | 6.9 → 8.5 | — | — | 16.9 → 16.5 | — | — | 16.4 | 5.1 | 6.4 → **6.4** |
| 1536×840 | set-language | 0.7 | 0.3 | 12.0 | 0.5 | 1.2 → 1.1 | 1.5 → 2.8 | — | — | 17.1 | — | — | 16.7 | 1.1 → 1.2 | 3.4 → **3.5** |
| 1536×840 | set-games | 0.7 | 0.3 | 11.9 | 0.5 | 4.1 | 3.1 → 3.9 | — | — | 16.8 | — | — | 16.5 | 3.6 | 5.2 → **5.3** |
| 1536×840 | set-activity | 0.7 | 0.3 | 14.7 | 1.2 | 5.1 | 6.9 → 7.3 | — | — | 16.4 | — | — | 20.5 | 4.7 | 6.7 → **6.7** |
| 1280×720 | home | 1.6 | 34.4 → 3.9 | 9.4 | 6.1 | 8.3 → 9.6 | 10.9 → 8.4 | — | — | 7.1 | — | — | — | — | 9.8 → **8.6** |
| 1280×720 | discovery | 1.9 | 34.2 → 3.3 | 1.7 | 9.5 → 9.4 | 14.9 → 11.1 | 6.6 | — | — | 7.1 | — | — | — | — | 11.3 → **7.6** |
| 1280×720 | quests | 1.8 | 34.4 → 3.5 | 9.9 → 9.7 | 7.4 → 6.4 | 55.2 | 2.1 → 2.5 | — | — | 7.1 | — | — | — | — | 34.6 → **32.8** |
| 1280×720 | inbox-0 | 1.6 | 34.4 → 3.8 | 9.9 → 9.7 | 4.7 | 29.2 → 27.6 | 11.6 → 8.4 | — | — | 7.1 | 37.7 → 36.6 | — | — | — | 21.1 → **18.3** |
| 1280×720 | inbox-1 | 1.6 | 34.4 → 3.5 | 9.9 → 9.7 | 4.8 | 19.2 → 11.7 | 11.6 | — | — | 7.1 | 22.3 → 12.0 | — | — | — | 15.7 → **10.0** |
| 1280×720 | inbox-2 | 1.6 | 34.4 → 3.5 | 9.9 → 9.7 | 4.5 | 8.9 → 8.2 | 11.6 | — | — | 7.1 | 6.6 | — | — | — | 10.2 → **8.1** |
| 1280×720 | inbox-3 | 1.6 | 34.4 → 3.7 | 9.9 → 9.7 | 4.5 | 8.5 → 7.8 | 10.4 → 8.4 | — | — | 7.1 | 6.3 | — | — | — | 9.9 → **7.7** |
| 1280×720 | inbox-4 | 1.6 | 34.4 → 3.7 | 9.9 → 9.7 | 4.5 | 8.0 → 7.1 | 10.5 → 7.4 | — | — | 7.1 | 5.5 | — | — | — | 9.7 → **7.3** |
| 1280×720 | set-account | 0.9 | 7.7 → 1.3 | 2.0 | 0.3 | 1.7 → 1.6 | 5.3 → 5.0 | — | — | 2.5 | — | — | 2.9 | 1.8 → 1.7 | 2.2 → **1.8** |
| 1280×720 | set-privacy | 1.8 → 0.9 | 9.6 → 1.4 | 2.5 → 2.3 | 1.6 → 1.2 | 11.4 → 11.3 | 13.0 → 11.7 | — | — | 2.5 | — | — | 3.2 | 9.7 → 9.5 | 8.2 → **7.6** |
| 1280×720 | set-msgperms | 1.7 → 0.9 | 1.1 → 1.0 | 2.2 | 1.7 → 1.2 | 8.9 → 8.8 | 9.2 → 8.2 | — | — | 2.4 | — | — | 2.9 | 7.5 → 7.4 | 6.1 → **6.0** |
| 1280×720 | set-notifications | 1.7 → 0.9 | 1.3 → 7.0 | 1.9 | 0.7 → 0.3 | 5.9 | 10.2 → 8.4 | — | — | 2.3 | — | — | 2.5 | 5.4 → 5.2 | 4.5 → **4.6** |
| 1280×720 | set-voice | 4.3 → 0.9 | 15.0 → 12.6 | 10.4 | 0.7 → 0.3 | 4.6 → 4.5 | 5.7 → 3.9 | — | — | 16.6 → 16.3 | — | — | 16.6 | 4.0 → 3.8 | 6.5 → **6.1** |
| 1280×720 | set-appearance | 4.3 → 0.9 | 15.3 → 13.0 | 10.6 | 0.7 → 0.3 | 5.9 → 5.8 | 5.1 → 3.2 | — | — | 16.3 | — | — | 17.0 | 4.9 → 4.6 | 7.2 → **6.7** |
| 1280×720 | set-accessibility | 4.3 → 0.9 | 9.8 → 8.1 | 10.9 | 0.7 → 0.3 | 6.4 | 4.9 → 3.6 | — | — | 16.3 | — | — | 17.5 | 5.3 → 5.1 | 7.3 → **6.9** |
| 1280×720 | set-system | 1.7 → 0.9 | 14.2 → 14.1 | 10.8 | 0.7 → 0.3 | 6.9 → 6.8 | 8.4 → 7.4 | — | — | 16.3 | — | — | 17.5 | 5.9 → 5.7 | 7.8 → **7.7** |
| 1280×720 | set-language | 2.6 → 0.9 | 16.5 → 14.7 | 10.9 | 0.8 → 0.3 | 1.5 | 4.7 → 3.4 | — | — | 17.0 | — | — | 17.7 | 1.7 → 1.5 | 4.9 → **4.6** |
| 1280×720 | set-games | 1.8 → 0.9 | 15.7 → 13.9 | 10.8 → 10.6 | 0.8 → 0.4 | 4.3 → 4.2 | 7.0 → 5.7 | — | — | 16.3 | — | — | 17.0 | 3.9 → 3.7 | 6.4 → **6.1** |
| 1280×720 | set-activity | 2.6 → 0.9 | 17.8 → 16.0 | 14.2 | 1.7 → 1.3 | 7.2 | 4.7 → 2.9 | — | — | 16.2 | — | — | 22.8 | 5.9 → 5.7 | 8.9 → **8.6** |

Reading: the two sides are minutes apart (the Phoenix render runs after the CDP pass), so new
messages, unread DMs at the top of the rail, presence changes and the inbox unread list count as
mismatch; the 1280 rail column (34 %) is a new unread DM that shifts every icon. The inbox
"before" rows at 1536 drew Phoenix on the last guild instead of Home (`real_shots` fix in lot 1),
so only their popout column is meaningful there. The settings "user panel" column is the strip of
user panel visible beside the settings layer.

### Fixed by wt/sweep3

| Gap (visibility) | Client reference | Fix |
|---|---|---|
| **High** — Friends "En ligne — 4" (client 60), Active Now with 3 cards, DM header / list without activities | the 2026 READY has no friend presences; they come in READY_SUPPLEMENTAL `merged_presences.friends`, whose events reached the app before the ReadyData and `apply_ready` wiped them; the session snapshot also dropped the supplemental and journal presence events when resuming | `app.rs` `apply_ready` keeps presences and activities, `PresenceReset` on READY_SUPPLEMENTAL (`nav.rs`, the payload is authoritative), `snapshot.rs` replays the presence events. Live 1280: 70 online vs the client's 64 a few minutes earlier |
| **High** (regression exposed by the fix above) — Active Now cards squashed to 2 px lines | the column is a scrolling flex column, its cards shrank | `friends.rs` cards `flex_none` |
| **Medium** — Activity privacy "Partager quand je commence à streamer" off (client on) | `ACTIVITY_PRIVACY_NOTIFY_SERVER_MEMBERS_ON_GO_LIVE_SETTING` = `notifications.notifyServerMembersOnGoLive` (field 26, default true); Phoenix read `notify_friends_on_go_live` (field 2) | `synced_map.rs` `act-stream` → `[7, 26]` |
| **Medium** — YouTube / video embeds in inbox, pins and chat show the description | `renderAll`: `Mg.has(embed.type) ? null : renderDescription()`, `Mg = {image, video, gifv}` (module 768570) | `chat.rs` `embed` |
| **Medium** — inbox / pins / search cards of a reply have no reply line | client cards render the referenced message line (spine, 16 px avatar, name at .64, preview) | `inbox.rs` `reply_line` (from `reply_preview`), avatar moved down 22 px |
| **Medium** — profile popout lists every role on its own rows | role list `Ql` (module 282c099a): first row 268 px, second row 268 − expand button − add button, the rest behind `pillButton expandButton` "+N" (padding 6 / 8, `interactive-text-default`, hover `background-mod-subtle`), expanded: chevron "Réduire les rôles" | `profile.rs` `roles_fit` + toggle (pills of the second row capped to its width), tooltips "Voir tous les rôles" / "Réduire les rôles" |
| **Medium** — text file preview stretched to 560 px | `.container_BOOBu-{display:grid;max-width:50vw;min-width:302px;width:100%}` in a shrink-to-fit column: a short file is 302 px | `plaintext.rs` card width = widest line (mono 14) + 18, clamped to [302, 50vw] |
| **Small** — text preview size "1 KB" / "MB" in French | `eg` footer and the bytes-left notice use module 255438 `up` (base 1000, rounded up, `bTzRR6` "{size} Ko", `cS889N` "{size} Mo", `yhEXX7` "{size} Go") | `plaintext.rs` `size_label`, keys `size.kb/mb/gb/decimal` |
| ✅ checked, no change — upload progress card and file card in "MB" in French | the upload row (`fileWrapper_-9z-XO`, module 905394) and the file card use `underscore.string` `filesize` (`["bytes","KB","MB","GB","TB","PB"]`, base 1024, 2 decimals, "," thousands): English units in every locale | `attach.rs` `human_size` already matched; GB/TB and the thousands separator added |
| **Small** — quests header: selected tab underlined | live `/quest-home` 13:13: both tabs `border-bottom: 0`, `::after` transparent, colour oklab(0.873) = text-default | `quests_page.rs` |
| **Small** — collapsed category hides the selected channel's thread | channel list `computeState`: selected channel (or parent of the selected thread) or expanded unmuted category → `activeJoinedRelevantThreads`, otherwise `activeJoinedUnreadThreads`; a channel with listed threads stays visible | `sidebar.rs` |
| **High** — "Voix connectée" panel with live controls while another session of the account (official client, phone) is in voice | the client's RTC panel follows its own RTC connection; other sessions only show in the channel's voice rows and the "Transférer la voix sur cet appareil" handoff item (`PlwgdU`) | `VoiceStateUpdate { own }` (voice state `session_id` = gateway session), `app.rs` sets `voice_channel` only for its own session |
| **Medium** — Friend rows show English presence summary ("Playing Valorant with Medal") | client draws the green activity icon and the activity name, followed by the custom status after a bullet | `friends.rs` (`cf929fbd`) |
| compare tool — inbox over Home rendered over the last guild | — | `real_shots` selects Home when the client was on Home |

### Gaps still open, by visibility

| Gap | Probable cause | Where |
|---|---|---|
| ✅ (wt/sweep3-fixes) **High** — settings sidebar: indented sub-entries under the open page ("Thème, Icône de l'appli, Messages, Boîte de discussion…" under Apparence; "Quelle activité je partage, Où je partage l'activité, Participation aux Activités" under Confidentialité des activités) with selection bar and click-to-scroll | 2026 settings sidebar lists the sections of the open page as an indented sub-list (selected marker bar) and scrolls the content to them | `ui/settings/view.rs`, `ui/settings/mod.rs` |
| **High** — quests hero: Phoenix shows the reduced-motion still (mountain art), the client plays `heroBannerAssetOrbs` webm `fbd6cf99…webm` (reduced motion off) | e74f6ab2 switched to the reduced-motion image; Phoenix has no webm playback, the first video frame would be closer | `ui/quests_page.rs` hero |
| ✅ (wt/sweep3-fixes) **Medium** — quests order: "Suggérées" sort (`sort_quests` with `Sort::Suggested`) already implemented and active | "Suggérées" sort of the client (`PINNED_QUEST`, stale, claimed, enrolled, expires) | `ui/quests_page.rs` |
| **Medium** — DM profile panel and the DM header activity subtitle ("Xcode • atombot.tech") | panel: compare tool (fixed: `PHX_MEMBERS` now also follows `profilePanel_`); subtitle needs the presence fix above, to re-check live (the DM view is not reopened) | `ui/dm_profile_panel.rs`, header |
| ✅ (wt/sweep3-fixes) **Medium** — profile popout: "+" add role button added (with `MANAGE_ROLES` check and second-row width budget); "Collection de jeux" card still open | widgets come from `GET /users/{id}/profile` (`widgets`), empty for the users tried earlier (popouts.md); "+" button added in `roles_block` | `ui/profile.rs` |
| ✅ (wt/sweep3-fixes) **Medium** — Active Now order: sorted by voice first, then activity start time descending, then user id | client sorts Now Playing by activity (voice first, then start time) | `ui/home/friends.rs` `active_now_column` |
| **Medium** — Registered games card in Activity privacy: client "Tu peux aussi partager l'activité de 17 jeux…, y compris Tom Clancy's Ghost Recon: Wildlands et Farming Simulator 15" with the game icon, Phoenix the generic text and a grid icon | needs the detected / registered games list (local game detection) | `ui/settings/pages.rs` `Related("apps")` |
| ✅ (wt/sweep3-fixes) **Medium** — emoji picker: removed "+ NOUVEAU" label and green frame under the server's top emoji row | client has no "+ NOUVEAU" block under the top server emoji row | `ui/pickers/grid.rs` |
| ✅ (wt/sweep3-fixes) **Small** — composer emoji button highlighted while the emoji picker is open | client tints the button (`INTERACTIVE_BACKGROUND_SELECTED` when `Tab::Emoji` is open for `Composer`) | `ui/chat/composer.rs` |
| ✅ (wt/sweep3-fixes) **Small** — pins: own server tag badge resolved from `app.me.tag` instead of old message author | Phoenix now uses `app.me.tag` when `m.author == app.me.id` | `ui/inbox.rs` `cozy_message_with` |
| ✅ (wt/sweep3-fixes) **Small** — profile popout 10 px higher than the client | adjusted popout anchor from `p.at.y - z(60.)` to `p.at.y - z(50.)` | `ui/profile.rs` `overlays` |
| ✅ (wt/sweep3-fixes) **Small** — search result cards ~5 px taller (messages 3 px scroll anchor rounding still open) | adjusted card vertical padding (`pt(6.)`, `pb(5.)`) and avatar top offset | `ui/search.rs` `result_card` |
| **Small** — Discover hero title wraps after "SUR" (client after "COMMUNAUTÉ") and the radial gradient | ABC Ginto Nord missing (known) | `ui/discovery.rs` |
| — Member list group counts, rail unread DMs, inbox contents | timing between the client capture and the render, not product gaps | — |

## Sweep 4 QA (2026-11-25, wt/sweep4-qa): pre-beta.2 pass

Client captures 15:45–15:51 at 1536×840, dpr 1.5 (sweep4.mjs), rendered with int-bin
ac72344d. The renders were made ~20 min after the captures (the client was restarted in the
meantime and CDP was down), so messages, member lists and presences count as timing.

| View | Title bar | Rail | Channels | Header | Messages | Composer | Members | User panel | Panel | Overall |
|---|---|---|---|---|---|---|---|---|---|---|
| #général | 1.5 | 3.2 | 3.0 | 2.4 | 17.8 | 3.6 | 25.0 | 6.9 | — | **13.1** |
| inbox (for you) | — | 3.1 | 3.0 | 3.1 | 20.3 | 3.6 | 24.0 | 6.9 | inbox 7.7 | **14.2** |
| search "merci" | — | 3.1 | 3.0 | 2.5 | 19.8 | 5.1 | — | 6.9 | results 15.9 | **13.3** |
| settings Appearance | — | 14.4 | 11.2 | — | 5.3 | 10.4 | — | 16.6 | nav 15.5 / content 4.5 | **6.4** |
| settings Voice & Video | — | 14.4 | 10.8 | — | 4.2 | — | — | — | nav 15.0 / content 3.3 | **5.5** |
| Discover | — | 3.0 | — | 7.9 | 9.3 | 38.5 (cards) | — | 6.6 | — | **8.7** |

### Fixed by wt/sweep4-qa

| Gap (visibility) | Client reference | Fix |
|---|---|---|
| **High** — Discover cards "0 membre" (client "4 625 124 membres"); search header "Pas de résultat" above 4 088 results | `ABMKx3` / `ZGVL3g` are ICU plurals (`[6,"members",{one,other}]`); Phoenix passed the pre-grouped string "4 625 124", which the plural node parsed as 0 | `i18n/messages.rs` `ungrouped` strips space / nbsp / narrow nbsp before parsing (plural and number nodes) |
| **Medium** — limited theme pill fixed at "90 JOUR(S) RESTANT(S)" (client 88) | module 211824: `Fg = 2026-12-13T18:00Z` (everyone), `zv = 2027-02-20T18:00Z` (`client_theme` perk, premium TIER_2), picked by module 189916 `E`; pill = module 366999 `SHORT_TIME_LEFT` (`/wnvqA`, `Jsq0XN`, `SBd+Bs`), rounding `d()` (days + 1 when hours are left, hours + 1 past 45 min, > 11 h → 1 day) | `settings/pages.rs` `limited_time_left` + keys `sp.ap.limited_left{,_h,_m}` |
| **Medium** — default theme swatches: Ash and Dark colours wrong (Ash #2b2b30, Dark #313338, Onyx #0c0c0e), selection ring 2 px `#8ca0fd` | `.ashIcon_IhyaN_` primary-600, `.darkIcon_` plum-20, `.onyxIcon_` black, `.lightIcon_` white; `.defaultThemeSelection_` inset 1 px interactive-text-default; `.selectionCircle_` 2 px outside, inset 4 px brand-500; `.checkmark_` brand-500 | `settings/mod.rs` `AppTheme::swatch`, `settings/view.rs` `theme_card`, sync button border |
| **Small** — "Thèmes de couleur" without the Nitro icon, 30 px too close to the limited swatch | live capture | `settings/pages.rs` `nitro_themes` |
| **Small** — mic test meter: 8 px wide grey blocks on a 12 px pitch, 16 px tall | module 782600 `C`: 8 px pattern, pill 4 × (h − 4) inset 2 px with round ends, frame `background-base-low` (`.notches_0EQxEk.gray_`), pills `--gradient-progress-pill-background`, lit part yellow-260 → green-360, size md = 28 px in Voice settings (pills 24 px), width rounded to whole cells | `settings/voice.rs` `meter` (canvas) |
| **Small** — search results cards 8 px wider than the client, no scrollbar | the results list is a thin scroller (`::-webkit-scrollbar{width:8px}`): the track takes layout width when it overflows | `search.rs` gutter + `thin_scrollbar_div`, back to top on page change |
| test — `/proc` scan test used `std::os::unix::fs::symlink` without a guard (CI builds `--all-targets` on Windows) | — | `streamer.rs` test behind `#[cfg(unix)]` |

### Still open

| Gap | Probable cause | Where |
|---|---|---|
| Speaker device / volume in Voice settings | local client settings (device "Logitech G PRO X", output volume), not synced | — |

### Cross-platform review (static, no Windows/macOS target installed)

Checked every `cfg` path added since v0.1.0-beta.1: `tray` (Linux module + stub), `tray/activation`
(Wayland, Linux-only deps), `notifications/dbus` (zbus Linux-only, `notify-rust` elsewhere:
`appname`/`icon`/`show` exist on Windows and macOS in 4.19), `updater` (per-OS asset, open command,
AppImage swap under `cfg(unix)`), `voice/call` (portal / hotkey fields and arms Linux-only),
`phoenix-voice` (`screencast`, `hotkey` Linux-only, PipeWire input under `cfg(unix)`),
`snapshot` / `msg_cache` (`mode()` under `cfg(unix)`), `streamer` (scan only on Linux), the
vendored `gpui-pre` rich text (no new platform trait method), `gpui-base` (pinned git rev on
origin). Runtime-only differences, no build break: `snapshot::other_owner` checks `/proc/<pid>`
(always "not running" on Windows/macOS), `fonts/fallback` and `tts` call Linux tools and fail
quietly, `diagnostics` reads `/proc`.

### Closed by wt/ci-ready (2026-11-25)

| Gap | Client reference / check | Fix |
|---|---|---|
| ✅ Settings sidebar not scrolled to the open page | web.js settings scroll manager: `scrollSidebarNodeIntoView(node, {animate, block: "nearest"})`, `padding: 12`, on every navigation; also after an accordion sub-list mounts (`onMount` → nearest). The user settings sidebar has done this since wt/details2 (`settings/view.rs` `reveal_probe`); the sweep shots were scrolled by the driver's `scrollIntoView({block:'center'})`. The server / channel settings sidebar (`standardSidebarView_jnk11m`, `sidebarRegionScroller`) has no scroll logic in the client | no change |
| ✅ Search group headers: channel-name emoji drawn as a text glyph | client capture `compare/sweep4/1536x840/search`: 💬 is a 22 px Twemoji image (1.375 em of the 16 px name) | `ui/search.rs` `header_name`: unicode emoji as `emoji_element` 22 px with 1 px side margins, highlights kept per text run |
| ✅ Session cache owner check used `/proc/<pid>` (always "dead" on Windows/macOS, two instances could write the journal) | — | `discord_backend/snapshot.rs`: `owner.lock` held with `std::fs::File::try_lock` (Rust ≥ 1.89) for the process lifetime; unsupported locking falls back to owning; tests for a held lock, a stale file and a reopened handle |
| ✅ `fc-match` spawned on Windows / macOS | a Homebrew fontconfig on macOS rebuilds its cache on first run, on the UI thread | `fonts/fallback.rs`: no `fc-match` on Windows / macOS, the locale's native script fonts (Yu Gothic, PingFang…) are put in the chain directly |

Cross-platform guards re-read: `tts` and `shortcuts/message` spawn `spd-say` / `espeak-ng`
(spawn error → `None`, waits on background threads), `diagnostics` uses `cfg!(target_os)`
branches and `Option` reads, `updater::open_path_in_os` is per-OS, `pulse` / `soundshare` are
feature-gated and return `None` without `pactl`. No `x86_64-pc-windows-gnu` check: no mingw and
/home at 92 %, so the review stayed static.

rustfmt: `cargo fmt --check` fails on main. Best `rustfmt.toml` is `max_width = 160` +
`use_small_heuristics = "Max"` (57 641 check lines, 2 086 hunks; 120 → 131 715, 200 → 69 804,
160 default → 88 635). Applied, it touches 331 files (+12 191 −30 916), 20 744 of them in
`theme/generated.rs`. Kept on branch `wt/ci-ready-fmt`, not submitted.

Re-check with int-bin 4e8677a5 (same captures): Appearance content 4.5 → 3.9 % (pill "88 JOUR(S)
RESTANT(S)", swatches and ring line up), Voice content 3.3 → 3.0 %, Discover cards now read
"4 625 124 membres" (the third card changed with the live featured list). Follow-up: Discover dots
`.dotOnline_9dea1c` green-360 / `.dotOffline_9dea1c` primary-300 (the member dot was text-muted),
mic meter height 28 (size md in Voice settings, measured 24 px pills).

## Glitch hunt 6 (2026-11-25, wt/glitch6)

Offscreen renders only, no CDP pass: `real_shots` with every location and setting given by env
(`PHX_GUILD` / `PHX_CHANNEL` = `#phoenix`, `PHX_THEME`, `PHX_DENSITY`, `PHX_FONT`, `PHX_AUDIO`,
`PHX_SIDEBAR_W`, `PHX_NO_SCROLL`, `PHX_NO_FEED_SYNC`, …) and a `node` stub first in `PATH`, so the
example never reaches the client. 1536×840 at dpr 1.5 (plus 1024×640 and dpr 1), Onyx, dark and
light; 26 user settings pages, 18 server settings pages, 15 popouts / menus / modals, Friends and a
member profile. Demo: `views` (11 scenes), `content_shots` (Onyx, light), `theme_shots` (dark and
light at 150 %, Onyx at 100 %), `dialog_shots`. Renders stay in `/mnt/windata/project-phoenix/compare/glitch6`.
References: CSS and JS of the local client cache (`~/discord-phoenix-ref/current-client`).

### Fixed

| Zone | Glitch | Client reference | Fix |
|---|---|---|---|
| Settings › Connected devices | rows with a log out ✕ started their text 3 px left of the current device row | the 48 px icon tile does not shrink | `settings/nested.rs` `session_row`: tile `flex_none`, text column `flex_1 min_w_0` |
| Settings › Voice & Video | long device names cut mid-glyph (« …par défaut: Lo, ») | `.value_uFkqU2{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}` | `settings/voice.rs` `select`: `text_ellipsis` |
| Profile editor | « Ajouter des pronoms » cut (« …pronon ») | dump `vanilla-settings-profile-editor-switcher`: `sizer_CJLDJI` 133.6 px, padding 0 5px, 14/18 | input padding 5 px, 14/18 |
| Settings › Nitro | super reactions and custom emoji had the same smiley, order swapped | module `f8ea05…` list `es`: Upload, `ReactionIcon` (KjrZ8Z), `SuperReactionIcon` (taMwg/), `ScreenArrowIcon`, `UserSquareIcon` | `pages.rs` perks + `super-reaction`, `user-square` icons (extracted, with fallbacks) |
| Settings and modal text inputs | text 22 px from the field edge and 14 px high (double padding) | `.input_0PLWhS{padding-inline:var(--space-12)}`, font inherited (16 px) | `modals::input_box`: input `px 0`, 16/20; channel name field keeps its 10 px inset |
| Server settings › Soundboard | local sentence without plural / no-break spaces | `HHCdvU` | `discord.yml` `gs.expr.sound_slots` |
| Quick switcher | scrollbar drawn over the guild names on the right | `.scroller_a1DYes{margin-inline-end:-17px}` | `switcher.rs`: host `mr -17`, rows keep 16 px for the bar |
| Guild rail | acronyms sized 16/14/12 (1–2 letters too small, 7+ overflowed) | module 67811 `A=[18,18,16,16,14,12,10]`, `A[len-1]` | `server_bar.rs` |
| Chat system messages | boost / pin / join / group sentences jumped below the author and were cut at the column edge (worse at 150 %) | inline text in the client | `chat/system.rs`: text fragments flow word by word (`message_ext::words`) |
| Chat « started a thread: » | empty thread name when the thread is not loaded | chunk `2b1d25…` `threadName: channelName ?? message.content` | fallback to `message.content` + « See all threads » |
| Event cards | voice events: empty location icon (`voice` is not an icon); external events: calendar | `LocationIcon` for a location, the channel otherwise | `events.rs`: `speaker` / `event/location` |
| View as role bar | back button with an empty icon (`chevron-left` does not exist) | module 792831 arrow, direction LEFT, 16 px | `arrow-left` built-in polygon |

Verified on new renders (int-bin c99c0823 for the first five rows).

### Checked, not bugs

- Server settings › Members « 1mo », « 9a », « 2 j »: client strings `XzBNbS`, `I1E8p6`, `GBLpQ8`.
- Threads popout « anciens fils » lowercase: client `wUNQ+w`.
- « View All Stats » in English on app widget cards: hard-coded in the client (`8e7aee…`).
- Boost page, long guild names wrap: `.guildDetails_rH8mBz` has no ellipsis.
- Thin scrollbars always visible in settings: `.thin_*` thumb has no fade.
- 🖤 role icon next to some names in chat: the Twemoji black heart.

### Open (not fixed here)

- Server settings › Accès is an empty page (title only, generic fallback): the page is not built.
- Server tag preview: the « Lily » sample hard-codes the tag text `GLAD`, and the « currently using » sample shows an empty avatar circle; the client chunk of this page is not in the cache, so not changed.
- Friends › Active Now: a game without an image (Brawlhalla) leaves an empty 32 px tile; client fallback not found in the cache.
- Demo only: `content_shots` shows a raw `<sound:900:901>` token in a message (markdown, wt/dm-call); `views forum` / `post` render the offline error state, and `dialog_shots` has no avatars (harness prefetch).

## Rail, channel list, user panel, Friends and DM list

### Before (25/11 20:55, `/channels/@me`, Pending tab open in the client, Online in Phoenix, client streaming in voice)

| region | mismatch | mean diff |
|---|---|---|
| title bar | 2.0 % | 0.8 % |
| guild rail | 1.7 % | 1.1 % |
| channel list (DM list + panel) | 10.4 % | 4.1 % |
| header (Friends tabs) | 18.8 % | 3.7 % |
| main (Friends list) | 13.2 % | 5.2 % |
| overall | 11.6 % | |

Most of the remaining channel list difference comes from states that only exist in the client at capture time: the Go Live panel above the voice panel, the active screen share button, the degraded connection icon and the self activity icon in the user panel.

| item | client reference | Phoenix | status |
|---|---|---|---|
| Guild rail positions | rail scan of the capture: every pill and folder within 0.8 px | `server_bar.rs` | ✅ |
| DM row activity line | `subText_TBNqol` > ActivityStatus (`container_HigqCD textXs_HigqCD`): green 12 px activity icon (#45a366), text body-xs/medium, `+N` counter in `--status-positive` when a custom status is shown, `•` then emoji and custom status; row name moves to 8 px from the top (34 px text column centred in 48 px) | `members::dm_sub_text` reuses the member list `sub_text` | ✅ |
| Friends Pending tab | same layout as the client capture once the tab matches (`PHX_TAB=pending`) | `home/friends.rs` | ✅ |
| Voice participants order | VoiceStateStore (module 607567) comparator `${selfStream?"\0":"\x01"}${(member.nick ?? getName(user)).toLowerCase()}\0${userId}`: streamers first, then name, then id as a string | `sidebar::voice_sort_key` | ✅ |
| Channel actions next to the voice timer | f4b2f93f8f92b799.css: `.iconItem_Wx6SnQ{display:none}`, shown on a selected row only with `.iconNoChannelInfo_Wx6SnQ`, on `:hover` `.channelInfo_Wx6SnQ{display:none}`; selected action icons `--icon-strong` | `sidebar::channel_row`: the timer and the user limit pill hide on hover, the actions stay hidden on a selected row that has one | ✅ |
| User panel subtext | `panelSubtextContainer_g6VwXp` > ActivityStatus: 12 px voice or Go Live icon (#45a366), 4 px gap, `•` in `--text-default`, custom status, 12/13 px medium | `members::self_sub_text` when in voice or with an activity, previous text otherwise | ✅ |
| User panel hover roll | `hoverRoll_Dq3ch1`: on hover `.default` fades out and `.hovered` (username) fades in, `transition: all .22s ease` | `states::hover_roll` (220 ms slide, see the wt/live-diff-a2 table below) | ✅ |

| Voice participant name vs server tag | `usernameContainer_kQORUk` flex 1 1, min-width 0; `chipletParent_kQORUk` flex 1 0, min-width 0, overflow hidden: the tag is clipped before the name ellipsis | `voice_panel::server_tag` shrinks and clips | ✅ |
| Noise suppression button | `button_9OQtot lookBlank_9OQtot`, background transparent in every state | no background when noise suppression is on | ✅ |
| "New unreads" top bar | module 616740 `renderTopUnread` / `renderBottomUnread` + bar 173860 `calculateState`: the items are `forEachShownChannel` ids of the text-like types (`95701.tr`: text, announcement, forum, media, threads…; no voice, `withVoiceChannels:false`), walked from the top (bottom for the bottom bar) until the first channel whose row is in view, keeping the first mentioned (`rG`) and the first unread (`rv`: not muted, `455234.Y` = hasUnread and unread setting ALL_MESSAGES) on the way, so each bar jumps to the farthest one; `isUnreadVisible` only looks at fully visible channel rows (threads are not rows), not muted, unread; top hidden when `topMention == null && (isUnreadVisible \|\| bottomUnread != null \|\| bottomMention != null)`, bottom hidden when `bottomMention == null && isUnreadVisible`; mention text `8zH0LJ` « Nouvelles mentions » without a count | `channel_unread_bar::pick` follows that walk: voice and stage channels are left out, a visible unread thread no longer counts as a visible unread (the likely cause of the missing #bienvenue top bar on 25/11), mentions show whatever is visible, the targets are the farthest channels, label « Nouvelles mentions » | ✅ JS + unit tests; the 25/11 state could not be replayed (both bars hidden on the client on 27/11) |

### Guild route (25/11 21:20, Project Phoenix campement call view, client in voice and streaming)

| region | mismatch | mean diff |
|---|---|---|
| guild rail | 2.2 % | 0.9 % |
| channel list | 8.2 % | 3.6 % |

## Channel header, chat, composer, member list and their popouts (25/11, wt/live-diff-b)

The client stayed on Friends, then on a voice call view, then on a text channel of the project server; no CDP pass changed its route. Offline passes compare the dumps `vanilla-guild-chat-busy` (1280×700, dpr 1.5), `vanilla-announcement` (1536×840, dpr 1.25), `vanilla-menu-message` and `vanilla-profile-popout` with `real_shots` on the same channel and size (`PHX_GUILD`/`PHX_CHANNEL`, `PHX_SIDEBAR_W=336`, a `node` stub so nothing reads the live client). The live pass (21:53, text channel, 1536×840 at 125 %) read the DOM and computed styles only; `Network.enable` saw no non-GET request.

Scores (`compare.py`, regions rebuilt from the dump DOM):

| view | region | before | after |
|---|---|---|---|
| dump `vanilla-guild-chat-busy` (honeypot) | header | 4.4 % | 4.0 % |
| same | composer | 3.7 % | 3.7 % |
| live text channel (project server, 21:54, int-bin 81ce1c58) | header / composer | — | 2.9 % / 1.4 % |

Messages and member list scores on the dumps are decided by data (two days apart: new reactions, statuses, the Activity panel the dump has and the offline render does not), so they are not used as a measure.

| item | client reference | Phoenix | status |
|---|---|---|---|
| Header topic emoji | `topic_pVbL3l` is markup: `img.emoji` 19×19 at 14 px (1.375 em), no margin | `emoji_text::markup_label` (images, no 1 px margin) | ✅ |
| Composer placeholder width | `placeholder_SHcSVi` 291 px in a 301 px `textArea_` (10 px short of the buttons) | `right(10)` | ✅ |
| Message menu quick reactions | `.wrapper_Qi4I4L{display:grid;gap:4px;grid-template-columns:repeat(4,1fr);justify-items:center;padding:8px}`, `.icon_Qi4I4L` 20×20, button 44×44 radius 8 | 4 equal cells, gap 4, padding 8, emoji 20 | ✅ |
| Member group header | `membersGroupName_86es0b` plain text span with ellipsis, count span `"\xa0— N"` (module 9fb052…) | one ellipsized span + nbsp count | ✅ |
| Reactions spacing | `.container_YUQhib` grid: `padding-top:.125rem`, `grid-row-gap:.25rem`; `.reactions_MmKct8{padding-top:.125rem}` | reactions `pt 4`, `pt 8` after media / embeds / forwards (video→pill 4 → 9 px, client 9) | ✅ |
| Message hover bar | live: `buttonsInner_SY_P2T` right 14, top −16, padding 2, border 0.8 px border-muted, radius 8, `box-shadow: rgba(0,0,0,.14) 0 1px 4px`, 28 px buttons, separator 1×24 border-subtle margin 2 4 | shadow was GPUI `shadow_sm` | ✅ |
| Profile popout « Collection de jeux » | chunk c476f1…: the first game-collection widget renders a breadcrumb (`innerContainer_s6-upr` padding 8 radius 8, title body-xs/medium text-strong, up to 4 icons 24×24 radius 4 gap 4 from `app-icons/{game}/{icon_hash}.png?size=48`, last icon at 50 % opacity with `+N` on `rgba(0,0,0,.6)` when more games) | `profile_games::breadcrumb`, `Game.icon` from `icon_hash`; games fetched like the client in one `GET /games?game_ids=…` per 20 ids (module 920495), the per-game requests never finished before the capture | ✅ 9 games, 3 icons + « +5 », same place as the dump |
| Message menu « Rappelle-moi » | current client `roMu1H` (« Rappelle-moi »); the 23/11 dump said « Créer un rappel » (`mJ3P0N`, now the save-type label) | « Rappelle-moi » | ✅ (no change) |

Open:
- ✅ Ellipsis one glyph early on texts with an emoji (composer placeholder « hone… » vs « honey… », member group « Dé… » vs « Dév… »). Cause: the taffy bridge rounded every measured leaf up to a whole device pixel, so a measured sibling (the « — 2 » count, the emoji pieces of a label) took up to 1 dp from the ellipsized box; « Dév » + « … » fits the client's 191.042 px box with 0.08 px to spare. Measured widths, text boxes and the ellipsis now round up to a LayoutUnit (1/64 dp) like Chromium (docs/gpui-patches.md, patch 14). The member groups also had a 1 px narrower box: the client's container is 268 + its 1 px left border (269), Phoenix counted the border inside the 268 (responsive.md, Member list).
- Mentioned message background: the client CSS (952007…, `.theme-dark`/`midnight`/`darker`) sets `--message-mentioned-background-default` to yellow 8 % and `--message-mentioned-background-hover` to yellow 4 %, used by `.mouse-mode.full-motion .message_SY_P2T.mentioned_SY_P2T:hover` and `.mentioned_SY_P2T.selected_SY_P2T` (524434…). No other layer: the only `:before` is the 2 px `--icon-feedback-warning` bar (4 px in forced colours), no rule for compact mode or role mentions. A dump at about half the yellow is the hovered or selected state, which Phoenix already draws (`chat.rs`, hover token). Light 8/12, high contrast dark 20/24, light 12/16, midnight 24/28.
- Mentioned message background: the token is `--message-mentioned-background-default` = yellow 8 % (computed 0.078 in every dump), but the `vanilla-announcement` capture shows half that on screen (10,6,0 vs Phoenix 19,13,0). Not changed until a live capture of a mentioned message.
- Hover screenshots: `Page.captureScreenshot` of the live client does not show `:hover` states while the window is not repainted (the DOM shows the bar). Hover values above come from the DOM.

### After (25/11 22:10, same guild route, int-bin 4ea4227b)

| region | before | after |
|---|---|---|
| guild rail | 2.2 % | 1.4 % |
| channel list | 8.2 % | 5.6 % |

The remaining channel list difference is live state: the client's Go Live panel (Phoenix is not streaming), the speaking rings and the voice members joining or leaving between the two captures.

### Still open

| item | client reference | status |
|---|---|---|
| DM name with a friend nickname | the 20:55 capture showed "oopi" in the client and "Initial" in Phoenix for the same avatar; the DM row (`tl`, 1b94b2e83f586720.js) reads `RelationshipStore.getNickname(otherUser.id)` | ✅ closed below (`computeChannelName`, module 47167; real_shots 506ce3a7 shows « oopi ») |
| Group DM status dot | the 20:55 capture shows a status dot on the front avatar of a group DM | ✅ closed below (`group_dm::group_avatar_with_status`, module 954376) |
| Soundboard "new sounds" dot | blue dot on the panel soundboard button, aria-label "de nouveaux sons sont disponibles" | ✅ closed below (module 258204, `soundboard::has_new_sounds`); do not scroll the soundboard popout (it writes settings-proto/1) |
| Hover roll slide | `.full-motion .default/.hovered { transition: all .22s ease }`, translateY ±107 % | ✅ closed below (`states::hover_roll`) |

## Read state bars, DM names, group DM dot, soundboard badge, hover roll (25/11, wt/live-diff-a2)

No CDP pass changed the client's route: it stayed on a text channel of the project server, so the DM list was not in the DOM. The references below come from the cached JS and CSS, the dumps and the 20:55 captures.

| item | client reference | Phoenix | status |
|---|---|---|---|
| Channel list unread bars | rows and bars count a channel as unread only when `hasUnread && resolveUnreadSetting(channel) === ALL_MESSAGES` (module 455234 `Y`); `resolveUnreadSetting` reads the channel flags (1024 all, 512 mentions), then the parent category, then the guild flags (2048 / 4096), then the notification level, and only with the new notification settings; a row with only mentions renders `UNREAD_LESS_IMPORTANT` (no style, no pill, 3d994642c869c81b.js) | `Backend::unreads_all` (inbox `Ctx::unreads_all`), used by `channel_unread_bar` and `sidebar::channel_row`; the bar decision is a pure `pick` with unit tests; `RUST_LOG=phoenix::unread_bar=debug` logs each candidate (unread, muted, unreads_all, mention_count, last_read, place) and the bars, `real_shots` prints it on stderr | ✅ likely cause of the missing top bar (an only-mentions channel counted as a visible unread); to confirm with the probe on the live route |
| Chat "N new messages since HH:MM" bar | `renderStream` (ceb73656db6e6af4.js): shown when `ReadStateStore.getUnreadCount(channel) > 0 && ready`, not tied to the scroll position; text `BctFH/` (same day) / `3wXb9P`, `wvtbbG` / `tHqbtg` (« N+ ») when `isEstimated`; the count is every loaded message newer than the ack, estimated when the ack is not in the loaded page (`rebuildChannelState`, web.b63d846b329d6470.js); clicking the text jumps to `ackMessageId` + 1 (`NewMessagesBarJumpToNewMessages`), « Marquer comme lu » acks | `commands::state::unread_since` at channel open (also for cached pages), bar no longer hidden at the bottom, `jump_to_first_unread`; « Marquer comme lu » goes through `select_channel`, which `real_shots` keeps without ack (`NO_ACKS`) | ✅ unit test |
| DM name with a friend nickname | `computeChannelName` (module 47167): DM `RelationshipStore.getNickname(id) ?? getName(user)`, `@` prefix for the placeholder | `dm_profile_panel::channel_name` in the DM list, chat header, composer placeholder, voice panel, call view, call popout and incoming call | ✅ |
| Group DM name with nicknames | unnamed group DM: `recipients.map(getNickname ?? getName).join(", ")` | `nav::group_name`: for a group without `name`, each recipient is `relationship.nickname` (non-empty) else the user's display name, joined with « , »; `ChannelExtra.group_named` now tells an explicit name apart, so the edit-group modal starts from the explicit name only | ✅ unit test `unnamed_group_names_use_friend_nicknames` |
| Group DM status dot | `PrivateChannel` (`ej`): status `ONLINE` when any recipient is online, else none; the diagonal facepile (module 954376 / 479abc9ff57878c5.js) then uses `DIAGONAL_FACEPILE_STATUS_32` and the regular 10px status at 22,22 (dump `vanilla-group-dm` #801) | `group_dm::group_avatar_with_status` in the DM list | ✅ |
| Soundboard "new sounds" dot | module 258204: new when `available && userId !== me && soundId > max(2026-11-01, guild joinedAt, lastDismissedObjectId)`; the last value is `PreloadedUserSettings.guilds(3).guilds(1)[guild].guild_dismissible_content_states(12)[870].last_dismissed_object_id(4)`; shown behind the user experiment `2026-10-soundboard-new-sounds-badge` and when the button is enabled; dot `dot_ncMHEz` 6px `--blurple-50` bottom right, icon cut by `masked_ncMHEz` (radius 5 at 100%−3px) | `nav::sounds_dismissed` / `sounds_seen` (READY and `USER_SETTINGS_PROTO_UPDATE`), `SoundboardSound.user`, `soundboard::has_new_sounds`, guild sounds requested once per voice guild; nothing marks them seen | ✅ unit tests; not visible in `real_shots` (no voice there) |
| Hover roll slide | `hoverRoll_Dq3ch1`: `.default` to `translate3d(0,-107%,0)` and opacity 0, `.hovered` from `+107%`, `transition: all .22s ease` | `states::hover_roll` (cubic-bezier .25,.1,.25,1, 220 ms), driven by the avatar group hover | ✅ |

Checked after the merge (int-bin 506ce3a7, `real_shots` at dpr 1.25): the DM list shows « oopi » where it showed « Initial »; the group DM row now lists the same three names as the client capture, so the 20:55 order difference was data, not code (nicknames inside unnamed group names: closed by wt/parity-sweep). The unread bar probe on the project server listed no unread candidate, so the #bienvenue case could not be replayed; run `RUST_LOG=phoenix::unread_bar=debug real_shots` on a guild with unreads to confirm the only-mentions cause. The Friends / DM score was not redone: the client had left `/channels/@me` and the user was using it.

## User settings, every page (25/11 21:30–22:40, client 1536×788 CSS at 1.5×, onyx/midnight, FR)

Client shots taken in the live settings overlay (non-GET requests blocked), Phoenix from `real_shots PHX_OPEN=settings:<page>` at the same size and scale (`int-bin` d68f10c9 before, 8f8ad040 / 067ea9ff / 3a1a1fb3 after). Region `settings content` = `contentBody_3Rvb29`, top of each page. Details in settings.md, « Live pass on every user settings page ».

| page | before | after |
|---|---|---|
| Compte | 1.3 % | 1.3 % |
| Données et confidentialité | 8.9 % | 5.2 % |
| Permissions de messagerie | 6.2 % | 3.6 % |
| Notifications | 4.3 % | 2.5 % |
| Nitro | 4.8 % | 4.8 % |
| Boost de serveur | 8.2 % | 4.4 % |
| Abonnements | 7.6 % | 3.6 % |
| Inventaire des cadeaux | 10.2 % | 4.7 % |
| Facturation | 2.2 % | 2.2 % |
| Voix & Vidéo | 3.0 % | 3.0 % |
| Apparence | 3.7 % | 3.7 % |
| Accessibilité | 5.0 % | 4.9 % |
| Système | 4.8 % | 4.1 % |
| Langue et heure | 1.0 % | 1.0 % |
| Jeux enregistrés | 3.9 % | 3.8 % |
| Confidentialité des activités | 5.0 % | 3.7 % |
| Applications connectées | 9.1 % | 8.0 % |
| Développeur | 1.2 % | 1.2 % |

Header 0.6–1.9 % on every page. Sidebar 3–6 % when the active page is near the top; 12–14 % for the lower pages only because Phoenix scrolls the nav to the active item while the client shot was taken with the nav reset to the top. Facturation briefly went to 3.8 % (its custom blocks still compensated the old 24px title padding) and is back to 2.2 % at 00dbe8b9; dividers lost their extra 10px at a0a8a436. What is left: gg sans vs Noto Sans wrapping, the Nitro rewards offer card on Connected apps (dynamic offer, not reproduced), the gift card Lottie art, the Notifications select width (client grid `auto auto`).

## Remaining text cuts (25/11 night, wt/text-rest)

References: dumps `vanilla-menu-thread`, `vanilla-group-dm`, `vanilla-guild-chat-busy` (1280×700, DPR 1.5) and their screenshots, CSS 249938 / 960179 from the local cache. Phoenix: `real_shots` on Aperture Sciences #〢🍯・honeypot (no ack in `real_shots`), int-bin 09c668da, same size and DPR.

| item | client reference | Phoenix before | fix | status |
|---|---|---|---|---|
| Member list border width | `container_86es0b` computed `border-left: 0.666667px` at DPR 1.5: Chromium floors border widths to device pixels, so the container is 268 + 1 dp (268.667 at 1.5, 268.8 at 1.25, 269 at 1 and 2); border pixel at x 1011.33 in the screenshot | `w(268 + 1)` border-box: content 268.333, border pixel at 1010.67, the chat column 0.67 px narrower | the 268 goes on an inner box, the border stays outside it (GPUI already snaps border widths in layout); members.rs and group_dm.rs | ✅ |
| Group DM member list | same `container_86es0b` (dump vanilla-group-dm: x 1011 w 269, `membersWrap` 268) | 268 with the border inside | same inner box as the guild list | ✅ |
| Custom status emoji | `.emoji_HigqCD{flex:0 0 auto;width/height:var(--custom-activity-status-emoji-size);margin-inline-end:0}` (14px in `textXs_`, 16px in `textSm_`), then `customStatusContent_HigqCD{gap:4px}`: emoji x 1330, text x 1348 w 148 (« Repose en paix Happy 03/1… ») | the emoji went through `status_label`, which wraps each emoji with 1px on both sides: text 2 px narrower, cut one glyph early | `members::status_emoji`, a 14/16px box with no margin, in the member list, DM list, user panel (all `sub_text`) and the friends list; test `custom_status_text_starts_after_the_emoji_and_the_gap` | ✅ |
| Composer buttons | `scrollableContainer_2x8-Qn{overflow-y:scroll}` with `::-webkit-scrollbar{width:12px}`: the buttons end 12px before the inner edge of the 1 dp border (gift ink at x 804) | `mr(13)` (written for an unsnapped 1px GPUI border): all five buttons 1.33 px to the left (gift ink 802.67), placeholder box 1 px short, « #〢🍯・hone… » for the client's « honey… » | `mr(12)` | ✅ buttons (int-bin 60857fbf: gift ink at x 804 like the client, member border at 1011.33); ✅ placeholder « #〢🍯・honey… » since int-bin b5af7669 (`real_shots` #honeypot, theme Onyx, 1280×700, DPR 1.5): no GPUI change was needed. With harfbuzz on the cached gg sans 400 16px (kerned), the CJK fallback for U+3022 / U+30FB (16px each) and Noto Color Emoji for U+1F36F (2550/2048 em), « honey » ends at 279.314 px in the 290.667 px `placeholder_SHcSVi` box and the ellipsis is 11.232 px, rounded up to 1079/96. That leaves 0.11 px, so the cut only works with the client's box width to the LayoutUnit, which patch 14 and `mr(12)` give. Vendored test `honeypot_placeholder_cuts_after_honey_like_the_client` (docs/gpui-patches.md, patch 14) |
| Composer vertical place | `channelTextArea_2x8-Qn{margin:0 0 10px}` (cozy `--space-xs`), border 0.667px, height 56 + 2 × 0.667; border pixel rows 949 / 1034 at DPR 1.5; the placeholder and the slate editor both have `padding: 17px 0`, so a draft and the placeholder sit on the same rows | `mb(xs - 0.8)`, written for the old unsnapped border: border rows 950 / 1035 (0.67 px low). The placeholder used `top(pad - 1.5)` and the typed text `py(pad)`, so a draft sat 3 device px under the placeholder (x-height rows 991 to 1001 against 988 to 998) | `mb(xs)`; the text column and the placeholder both start `TEXT_LIFT` (1px) above the 17px pad (`pt(pad - 1)`, `pb(pad + 1)`, so the height stays 56) | ✅ int-bin 7c1f2714: border rows 949 / 1034 like the client; ink of the placeholder and of a `PHX_DRAFT` « Envoyer un message dans » on rows 981 to 1003, x-height from 988 (the client: 981 to 1004, x-height from 988) |
| Server tag chips | `tagText_k8jyN7` on one line (« GLAD », « Zzz ») | « GLA » + « D », « Zz » + « z » painted over the status line: the text rewrapped at its snapped paint width | GPUI patch 15 (rewrap slack, docs/gpui-patches.md) | ✅ int-bin 60857fbf: « GLAD », « Zzzz » on one line; the custom status reads « Repose en paix Happy 03/1… » like the client |

Sweep on the same capture (client dump vs Phoenix): member group « 💻 ─ ☲☲ Responsable du Dév… — 2 », names « Hopinett… », statuses « "Moin de paprasse, plus de terr… », header topic « Honeypot — … », channel names and the user panel cut at the same glyph. Not swept here: thread and forum titles, server names in tooltips (no `real_shots` view on the same data).

## Glitch hunt 7 (2026-11-26, wt/glitch7)

Offscreen renders only, no CDP pass (int-bin 60857fbf, then 594490f1 for the after shots): `real_shots` on `#phoenix`
with a `node` stub first in `PATH`, Onyx / dark / light, 1536×840 at dpr 1.5, 1920×1080, 1280×800 at dpr 1,
940×700 and 1024×1100; `PHX_OPEN` modview, createevent, addserver, follow, followok, statuspicker, guildmenu,
profilemenu, profileoverflow, 11 user settings pages; member profile popout and modal; hover on a message, the mic
(Lottie frames 120 / 400 ms) and the rail; the #phoenix test message 1575263335010799749 (text previews, audio,
spoilers). Demo: `content_shots` at 150 % (AutoMod, system, call, spoiler, mosaic, audio jumps), `call_view dm chat`,
`call_view`, `views` (profile-modal, members, dm-profile, events, boost, guild-menu), `component_shots`.
Renders stay in `/mnt/windata/project-phoenix/compare/glitch7`. References: CSS and JS of the local client cache.

### Fixed

| Zone | Glitch | Client reference | Fix |
|---|---|---|---|
| Audio player | tinted generic file glyph instead of the audio file icon | `.audioMetadata_FJ99sr:before{background-image:url(/assets/a6db1d05d35f13d0.svg);background-size:100% auto;height:40px;width:24px}`, opacity .6 | `chat/attach.rs` `audio_card`: the asset, 24 wide in a 24×40 box |
| Audio player | control bar too light (black 28 %), worst in light theme | `.audioControls_FJ99sr{background-color:var(--opacity-black-40)}`; black 28 only under `.background-opacity-*`, the in-game overlay classes (`web.*.js` `LOW/MEDIUM/HIGH/FULL`) | `chat/voice_message.rs` `attachment_controls` |
| Mod View | action tiles drawn edge to edge, so disabled tiles (Kick / Ban / Timeout) filled the whole cell | `.bottomRowAction_ebzPvR .innerBottomRowAction_ebzPvR{width:calc(100% - 16px)}` | `mod_view.rs` `action`: outer `px 8` |
| Text file preview | in a narrow chat (940 px window, or 150 % zoom) the card shrank below its footer: file name and size vanished, the ⋯ button was cut | `.container_BOOBu-{max-width:50vw;min-width:302px;width:100%}` (min wins over max) | `chat/plaintext.rs` `render`: `min_w 302` |

Verified on new renders (int-bin 594490f1).

### Checked, not bugs

- AutoMod footer « Actions · Jump to Surrounding · Report Issue » runs past a very narrow column: `.centeredRowContainer_bbJCi7{display:flex;flex-direction:row}` has no wrap in the client either.
- Audio seek bar shrinks to nothing in a very narrow column: `.mediaBarWrapper_s1cr3c{flex:1 1 auto}` has no minimum.
- Mod View in the light theme sits in a dark frame: the member's profile theme (themed container, `profile_theme::composite`), like the client.
- YouTube embed play / open pill: `.wrapper_lj7V-K` black 60 %, 48 px, icons at .6 opacity, as drawn.
- `PHX_OPEN=settings:keybinds` shows the System page under the « Raccourcis clavier » title: the 2026 sidebar has no Keybinds entry, the page is only an internal alias.

### Open

- Profile role pills put the role icon after the name; the client chunk of the profile role list is not in the cache, so not changed.

## Glitch hunt 8 (2026-11-26, wt/glitch8): popouts and modals

Offscreen renders only, no CDP pass: `real_shots` on `#phoenix` with a `node` stub first in `PATH`, 1536×840 at dpr 1 and 1.5,
dark and light, int-bin 80a9684b / f8bcd4e8 for the before shots, 7aea879c then later builds for the after shots. Views: member
profile popout and modal (bot), own profile modal, `PHX_OPEN` modview, tagpopout, follow, createevent, addserver, poll_create,
acct:username / password / email, remindcustom, nickname, report, settings:changelog, settings:family, statuspicker, invite,
leave, customstatus. Renders stay in `/mnt/windata/project-phoenix/compare/glitch8`. References: CSS and JS of the local client cache.

### Profile role list

The glitch 7 open line is closed: the list is in the cache (chunk `282c099a…`, module 166005, CSS module 434367 `_xL50-d`).
The pill `Y` renders, in this order: role dot (`roleCircle_7RsUnm` 12 px, margin 0 4) or the `dot` role style, a 12 px
`LinkIcon` for a connection role (`connectionRoleIcon_xL50-d`, `--icon-default`, margin-inline-end 4), the name
(`body-xs/normal`, `--text-default`), the role icon (`roleIcon_xL50-d`, 12 px, margin-inline-start 4), then the remove button
(`XSmallIcon` xxs). So the icon after the name is the client order. The same component serves the popout and the full profile.

| Item | Client | Fix |
|---|---|---|
| Connection role | `R && LinkIcon` before the name | `profile.rs` `roles_block`, width in `role_pill_w` (test `role_pill_reserves_dot_link_name_icon_and_remove_like_the_client`) |
| Expand / collapse | `ChevronSmallLeftIcon` size `xs` = 16 | `chevron-small-left` 16 (was a rotated 12 px chevron) |
| Add role | `.addButton_xL50-d`: no border, no hover background, colour only; `PlusSmallIcon` size `sm` = 18 | borderless, `plus-small` 18, hover `--interactive-text-hover` |

### Fixed

| Zone | Glitch | Client reference | Fix |
|---|---|---|---|
| Profile activity card | a streaming bot showed « En direct sur J'aime bien ce point violet. » over an empty 48 px tile | module 770689: STREAMING header `4CQq9Q` « Streame {name} » only for Twitch / YouTube, else `Jpkr/q` « En direct »; module 802880 `t0`: image and elapsed time only when `!user.bot`; title = details ?? name | `profile.rs` `activity_header`, `activity_block(.., bot)` (test `streaming_header_names_only_known_platforms`) |
| Profile modal tabs | « Liste de souhaits » always shown, « 0 ami en commun », mutual tabs on your own profile, « 15 serveurs en com… » cut | chunk `dd9e93b2…` tab list: wishlist only for yourself or a public non-empty wishlist (module 419341), mutual tabs only for other users; labels from modules 837531 / 186272: 0 gives `n9g3ay` / `jpY0X5` | `modal_card_inner` tabs, `mutual_tab_label` (test `empty_mutual_tabs_use_the_client_none_label`) |
| Own profile modal | « Envoyer un message » (disabled), shop and ⋯ | module 74525 `D`: for yourself a full width primary « Modifier le profil » (`s5vZlQ`, PencilIcon), with the server / main profile menu inside a guild | `modal_actions` + `profile_toolbar::edit_profile_button` (test `own_profile_modal_shows_only_edit_profile`) |
| Account popout, light app theme | name, username and bio dark on the dark profile theme, menu cards white | the themed card takes the profile theme scheme (`custom-user-profile-theme`), like the member popout | `status_picker::render` runs inside `profile_theme::scoped`, inner overlay `profile_theme::overlay` |
| Account edit modals | field text 23 px from the edge, 14 px | `.input_0PLWhS{padding-inline:var(--space-12)}`, font inherited 16 px (same as glitch 6 `input_box`) | `account_edit.rs` `input`: input `px 0`, 16/20 |
| Server tag popout | always the load error card in `real_shots` | `GET /guilds/{id}/profile` | the `real_shots` read-only backend now forwards `guild_profile` (GET only) |
| Server profile date | « Depuis le Jan 2026 » (English month, boost string `lY2Bur`) | module 107123 `P`: `toLocaleDateString(locale,{year:"numeric",month:"short"})`, string `zb2Q56` « depuis {createdAtDate} » / « Est. » | `guide::established` shared with `tag_popout`, `guide.established` → `zb2Q56` (test `established_uses_the_localised_short_month_and_year`) |

### Checked, not bugs

- Poll creation « Post » in French: the client string `JOj8Zk` is « Post » in fr too.
- Custom reminder: the Heure field stays empty until the current time is a quarter hour (`lookupByValue`, chunk 549208, see inventory).
- Follow modal « Suivre » disabled until a server and a channel are chosen; create event « Suivant » disabled until a place is picked.
- Bot profile: the white APERTURE custom emoji in the bio is a white image, invisible on a light card in the client too.

### Open

- Email change « Envoyer le code de véri… » is cut in two equal width buttons: the mana button text ellipsizes in the client too (`.buttonChildren_F7qvhY{text-overflow:ellipsis;white-space:nowrap}`), but the modal component (and so its button layout) is not in the cache.
- Invite modal: Phoenix still draws the legacy invite modal (centred 24 px title with a loose line height, no friend list). The 2026 client uses a mana list modal (`p.a`, title / subtitle / search / friend rows, chunk `a6539095…` `getHeaderContent`); this is a rebuild, not a glitch.
- Mod View role pills have no role icon; the Mod View chunk is not in the cache, so not changed.
- Server tag popout without a splash shows a flat grey banner; the client fallback was not found.
- Bot bio `:rocket:` stays as text; whether the client bio parser turns shortcodes into emoji was not checked.

## Glitch hunt 9 (2026-11-26, wt/glitch9): main views

Offscreen renders only, no CDP pass: `real_shots` with a `node` stub first in `PATH`, int-bin 0e128aa4 for the before shots and
ca3d2849 for the after shots. Views: Aperture `#général` (dark, light, Onyx, each with and without High Contrast; compact at 1920×1080),
`#phoenix` Onyx 1920, `#honeypot` light 940×700, `annonces` compact, the `suggestions` forum (dark 1280, light 940), the thread
« [Test Phoenix] fil autonome », a DM (dark cozy, light compact), Friends (dark, Onyx HC 1920, light « Tous » at 150 %). Zoom 125 % and 150 %
are rendered as the same CSS viewport (1024×640 at dpr 1.875, 853×533 at dpr 2.25). Renders stay in
`/mnt/windata/project-phoenix/compare/glitch9`. References: CSS and JS of the local client cache, `fr.js` strings, gg sans font files.

### Fixed

| Zone | Glitch | Client reference | Fix |
|---|---|---|---|
| Reactions, High Contrast | every High Contrast render panicked (`hover style already set`): the HC branch called `.hover` a second time on reactions and the add reaction button (debug builds abort, release builds dropped the normal hover) | `.high-contrast-mode .reaction_MmKct8` border `--border-subtle`, hover `--border-strong` (themes.md) | `message_ext.rs` `reactions`: one `.hover` whose border depends on HC |
| Compact mode | the first line started at an estimated header width: too far right on most rows (up to ~70 px), and over the server tag for a CJK name (« Coucouu » drawn on « LJDC ») | compact header is inline text, content follows after the name's 4 px | `chat.rs`: the header width is measured by a canvas probe (`compact_header_probe`, per message, refresh on change); the indent uses U+00A0, which gg sans has (3.52 px regular, 3.84 px italic at 16 px, mixed to land within 0.16 px). gg sans has no U+2009 glyph: the old thin spaces were measured at 1 px and drawn by a fallback font about 3 px wide. Test `compact_indent_mixes_regular_and_italic_spaces_to_hit_the_width` |
| Discovery notice | at 940 px the text was clipped (« …de ce salon p ») and « En savoir plus » dropped to its own line | `bar_8PBDGr` text is inline: one paragraph, the link wraps with it | `chat.rs` `discovery_notice_text`: one `StyledText`, underline highlight on the link |
| Video attachment | in a column narrower than the video, the card kept its height and cropped the poster (annonces video 1280×720 shown 505×310) | media items keep `aspect-ratio` under `max-width:100%` | `attach.rs` `video_card`: `aspect_ratio(w / h)` instead of a fixed height, like `media_image_with` |
| Forum cards | a two-line title was cut mid-glyph at the card edge | chunk `6651e9581db5d17c.js` `heading-lg/semibold lineClamp:2` (ellipsis on the last line) | `forum.rs`: `line_clamp(2)` + `text_ellipsis()` (GPUI only ellipsizes a clamp with `text_overflow`) |
| Role gradient names, light theme | gradient names kept full brightness | `.theme-light .usernameGradient_HG2xvK{filter:brightness(.85)}` (`447505.1c681faad626d654.css`) | `fonts::name_text_with` scales the stops by 0.85 in the light theme (display name styles keep theirs, `name_text`). Test `light_theme_role_gradient_stops_are_dimmed_like_the_client_filter` |

Verified on new renders (int-bin ca3d2849): the three High Contrast renders draw, compact rows start 4 px after the header (guild and DM),
the notice wraps on three lines, the video shows the whole frame, the pinned forum title ends with « je… ».

### Checked, not bugs

- `#général` header shows the topic dot with no text: the topic is U+E0078 (an invisible tag character), not empty, so the client shows the divider too.
- « Tous les amis - 199 » with a hyphen while « En ligne — 23 » has an em dash: `fr.js` `rHRrhC` is « Tous les amis - », `BagU2U` « En ligne — ».
- Forum search at 940 px with the member list: the field is about 20 px wide between the search icon and the fixed buttons, so the placeholder has no room (same flex layout in the client).
- Channel title cut without an ellipsis at 940 px: `titleWrapper_` is flex-shrink 0 and clipped (responsive.md).
- Header wrapping at 125 %: the timestamp starts the next line 4 px in, the client's inline `.timestamp` margin.

### Open

- The client member list shows an « Activité — n » card section on busy guilds (dump `vanilla-guild-chat-busy`); Aperture had no card in these renders, not checked.
- A DM name with a dark to light display name gradient (Clem) starts almost black on the dark theme in the message header: fixed, see the display name style row in `chat.md` (the client lifts each colour to a minimum contrast and only shows the effect on hover in message headers).

## Sweep 5 (2026-11-26, wt/sweep4): rail, channel list, member list, user panel at three sizes

Live client: Project Phoenix server, `discussion` (the user moved between the call view, `discussion` and a DM during the pass, so
each CDP read was a single read-only `Runtime.evaluate` followed by a 5 minute pause). Client 1536×840 CSS px at dpr 1.25 (zoom 125 %),
Onyx, cozy, `enlarge-target-size`. Phoenix: int-bin 2a7e665d, `real_shots` at dpr 1.5, live copy of the client's location, scroll,
sidebar width and folders (`compare.py`), then 1280×720, 1600×900 and 1920×1080 with the same guild and channel. The client window is
never resized, so the three sizes are compared on the fixed width regions only (rail and channel list anchored top left, user panel
bottom left, member list top right); the fluid chat column is covered by `responsive.md`. Renders stay in
`/mnt/windata/project-phoenix/compare/sweep5`.

Scores at the client size (`compare.py`, mismatch %): title bar 3.4, rail 34.2, channel list 5.6, header 2.6, messages 19.7 (the
two sides were not on the same messages: the user scrolled), composer 2.2, member list 8.1.

After (int-bin 88079e2d, same route, 16:00): title bar 2.2, rail 4.8, channel list 3.9, header 1.1, messages 13.7 (new messages since the
client shot), composer 1.3, member list 2.9. The voice panel lines up row for row; Phoenix shows « Déconnecté » offscreen because
`real_shots` does not join voice.

### Fixed

| Zone | Gap | Client reference | Fix |
|---|---|---|---|
| Channel list | « Nouveaux messages non lus » pill missing when the unread channel is the first row, cut by the top edge | `isChannelVisible` (chunk `6b05ca0762af596a.js`): for the top bar a row is visible only when its top is below `scrollTop`, for the bottom bar only when its bottom is above `scrollTop + offsetHeight`; `isUnreadVisible` uses `isItemVisible(…, true)` (fully inside) | `channel_unread_bar.rs` `place_of`: a partly hidden row counts as above or below. Test `partly_hidden_rows_count_as_hidden_like_the_client` |
| Voice panel | panel 4 px shorter than the client: connection row 32 instead of 40, ping and buttons not offset | experiment `2026-09-a11y-chassis-target-size` (module 657043, buckets 1 and 2 set `.enlarge-target-size` on `<html>`); CSS `587669.09c9393812bb8ea9.css` (`rtcConnectionStatus` height auto, label 24, `labelWrapper` mt −1 pb 1, `largeClickablePing` mt 6), `593586.d3f89ba5001fe235.css` (`container_9U6LUN` gap 10, padding-top X − 2, `connection` / `inner` height auto, `voiceButtonsContainer` pt 4). Live boxes: container 104.8 tall, ping 32 at +6, label 24 then subtext 16, buttons row at +58, separator at +104.8 | `voice_panel.rs` `panel_box` / `enlarge_targets` (bucket ≥ 1 from `GET /experiments`). Tests `enlarged_voice_panel_lands_on_the_client_boxes`, `target_size_experiment_turns_on_for_both_enabled_buckets` |
| Voice panel | three bars in the ping square; « Voix connectée » semibold, subtext regular | live SVG in `largePing_4LJE6I`: `ConnectionFineIcon` (3 arcs). Map in chunk `web.b63d846b329d6470.js`: FINE → `ConnectionFineIcon`, AVERAGE → `ConnectionAverageIcon`, BAD → `ConnectionBadIcon`, UNKNOWN → `ConnectionUnknownIcon`, default colours `ICON_FEEDBACK_POSITIVE` / `_WARNING` / `_CRITICAL` / `INTERACTIVE_ICON_DEFAULT`; 18 px (`largePing` 18 + padding 7). Computed styles: label `text_Ybsmt1` 16/20 weight 500, padding-top 4 (enlarged); subtext 12 px weight 500 | `voice_panel.rs` `connection_icon` (icons extracted at runtime, `names.rs`), label and subtext `FontWeight::MEDIUM`, label padding 4 when enlarged. Test `connection_icon_follows_the_client_quality_map` |
| User panel | own avatar Online while the account is Do Not Disturb (settings proto field 11 = `dnd`, read by `GET /users/@me/settings-proto/1`) | PresenceStore (chunk `web.b63d846b329d6470.js`, `PRESENCE_UPDATES` → `v`): `if(n===_.default.getId())return!1`, guild presences of the current user are dropped; the own status only comes from the sessions and the settings | `convert.rs`: guild `PRESENCE_UPDATE`, `READY_SUPPLEMENTAL` merged presences and member chunk presences skip the current user (status and activities). Test `guild_presences_never_override_my_own_status`. The status from READY (`self_status`) is emitted before `apply_ready` sets `me`, so it landed in the presence map and the panel kept the default Online: `apply_ready` now takes it from there (test `own_status_sent_before_ready_reaches_the_user_panel`) |
| Experiments | the enlarged voice panel and the soundboard « nouveaux sons » dot never turned on: `GET /experiments` has neither `2026-09-a11y-chassis-target-size` nor `2026-10-soundboard-new-sounds-badge` | 2026 experiments are apex experiments (`mj({kind:"user"})`): the client reads `READY.apex_experiments` (and `GET /apex/experiments?surface=2`), `setExperimentAssignments` tuples `[hashedName, variantId, flags, revision, trackedVariantId, config]` under unit `1` (user). This account: target size `[3012809029, 1, 0, 2]` (variant 1: enabled, `rtcLargerTarget` off), badge `[2094783262, 1, 2, 1]` | `phoenix_api::rest::experiments::apex_assignments` / `set_apex` from READY (live and snapshot), merged first into `user_experiments`. Test `apex_assignments_read_the_user_unit_variant` |
| Member list | activity cards: black background, square game thumbnail at the top of the card, « Série de 5 j consécut » cut without an ellipsis | CSS `237427.f55067b3f6b591b5.css` `.container_g7esM5` (`align-items:center`, `--background-surface-high`, hover `--interactive-background-hover`, border `--border-muted`), `img.thumbnail_g7esM5` cover; dump `vanilla-guild-chat-busy` #1550–#1574 (`contentImage` radius 4, thumb 48 at row + 2, badge text `lineClamp1` with ellipsis); `84194.47b4539899f7e613.css` `.badgeContainer_CTWncy` overflow hidden | `members.rs`: tokens, `items_center`, `img` rounded 4 (GPUI does not clip an image to its parent's radius), badges `min_w_0` + `overflow_hidden`, the time badge keeps its width |

The other `.enlarge-target-size` rules (category `addButton_yOdhm_` 24 px, DM header buttons, DM row close button, channel row
`iconItem_Wx6SnQ` padding 4, `channelInfo` pe 4, `children_TNyK-N` margins) were already drawn at the enlarged size from the dumps of this
account: same positions on both sides. They are not switched off when the experiment is off.

### Open (not fixed here)

- Rail: the client showed one DM with an unread mention under Home (badge 1); Phoenix's render 30 s later did not. The user opened that DM
  a few minutes later, so the state is gone and the cause (read state `mention_count` or the DM list) could not be traced.
  Sweep 5b: the client rule is `PrivateChannelReadStateStore.getUnreadPrivateChannelIds` (web bundle module 131677): the
  `PrivateChannelSortStore` order (favourites first, then the newest `lastMessageId`; message requests and spam left out), kept when
  `ReadStateStore.getMentionCount > 0`, cut at 20; the rail (`tq`) drops the DM call and puts it first. There is no mute filter: a muted
  DM with a real mention still shows. Phoenix hid every muted DM, so a muted DM with a mention never reached the rail; it now follows
  the client (`server_bar.rs` `rail_dm_ids`, six tests). Live counting follows `ReadStateStore` too: in a muted DM only a real mention
  counts (`eD` = private and not muted), test `muted_dm_counts_only_real_mentions_like_the_read_state_store`. The DM tile now gets the
  call badge (`oi`) and the selected pill. ✅ (wt/active-now) « Messages favoris » order: `PrivateChannelSortStore` (module 645959) puts the DMs whose DM settings override has `FAVORITED` (2048, `isMessagesFavorite` = `getChannelOverrides(null)[id].flags`) in a FAVORITE section before the DEFAULT one, each by newest `lastMessageId`; `server_bar.rs` `rail_dm_ids` takes the same partition (`NotifyPrefs::is_messages_favorite`), tests `favourite_dms_come_first_in_the_private_channel_order`, `favourite_dms_read_the_favorited_override_flag_of_the_dm_settings`.
- The three window sizes were rendered without the client's live state (sidebar width, folders, voice) when the CDP pause was on, so their
  fixed region scores (rail + channels 20–22 %, user panel 12–16 %, member list 12–16 %) mostly measure that, not a layout gap. At the client size
  with the live state the same regions score 11.7 / 9.3 / 8.5 %.

## Visual sweep (2026-11-28 night, wt/visual-sweep): pixel compare at the live window size

Method: the client is captured at its own window size, never emulated (1280×700 CSS at devicePixelRatio 1.5, Onyx,
FR, page zoom 125 %), with the driver `~/discord-phoenix-ref/vsweep/{vsweep.mjs,run.py}` (a copy of the sweep 5 driver
without `Emulation.setDeviceMetricsOverride`). Every pass runs under the CDP lock with `Network.enable` and a `Fetch`
block on `*/ack`, `settings-proto` and `migrate-command-scope`; the only non-GET requests seen were `POST /science`, plus
`POST /guilds/:id/members-search` once when the server settings « Membres » entry opened the member-safety route (that
page was dropped). From 02:30 the client window stopped painting (`requestAnimationFrame` stalled, screen off):
`Page.captureScreenshot` still renders a frame, but `Input.dispatchMouseEvent` hangs, so the later passes click with
`element.click()` in the page (`FAKE_INPUT=1`) and force frames with tiny screenshots before the capture.
Phoenix is `int-bin/real_shots` at the same size and dpr, from the saved client state (route, scroll anchors,
sidebar width, folders, member list, theme). Score = `compare.py` `mismatch_pct` (share of pixels off by more than
24/255). Images stay in `/mnt/windata/project-phoenix/compare/vsweep/` (private).

Caveats: the voice panel is absent or « Déconnecté » in real_shots while the client sits in a call (fixed in
real_shots by 8cc805f7, after these scores), and the messages region also counts messages that arrived between
the capture and the render.

| View (1280×700) | Region scored | Before (int-bin eb489280) | After (a2cae139) | Overall before → after | Fix |
|---|---|---|---|---|---|
| #général chat | messages | 9.8 | 6.6 | 6.5 → 5.9 | last message keeps the 24 px the 40 px `scrollerSpacer_` leaves above the composer (it overlaps the form by 16) |
| Member profile popout | popout | 32.8 | 7.6 | 13.8 → 6.8 | popout left of the clicked `member_` row (x = row − 16 − 300) at its top, nudged into the window like `nudgeTopAlignment` (top = vh − ceil(h) − 12 when it overflows), real height instead of a fixed 540 |
| Pinned messages | popout | 15.3 | 6.5 | 8.9 → 6.5 | right edge on the pin button (was 10 px off), top on the header bottom, max height 80vh (was 80 % of the chat column) |
| GIF picker | popout | 12.6 | 12.7 | 7.4 → 8.1 | search text 16 px with no inner padding, tiles rounded 5 (`result_H8nOze`); body background `container_rfDq2B` = `--background-base-lower` follows (b835ca7a) |
| Emoji picker | popout | 12.3 | 12.3 | 7.3 → 7.7 | same search fix; the Nitro rail item (« Voir les émojis Nitro ») shows for Nitro users too (3e66fbc1, not in this int-bin) |
| Sticker picker | popout | 8.1 | — | 6.2 | inspected sticker ring (`stickerInspected_5z8c5P:before`, 2 px `--interactive-text-hover`, r12), stickers keep their aspect, inspector shows the server icon (module 22fbc347793fb277 `tl`) |
| Threads | popout | 7.4 | — | 4.9 | — |
| « + » menu | popout | 8.0 | — | 5.8 | — |
| Inbox, unreads | popout | 14.6 | — | 10.2 | tab « Messages marqués » = `2pAkDA` (was the « Sauvegardé » string), day divider pill (`divider_4TfzT5` + `content_4TfzT5`, black pill, margins 16), 8 px stable scrollbar gutter (3e66fbc1) |
| Friends (Pending), DM list, Active Now | overall | 8.3 | — | 8.3 | DM rows 38/42/48 by density (`layout_TBNqol`), plated close space only while hovered or selected (`eZ = nameplate && (selected‖hover‖focus)`), Active Now title margin collapsed into the 12 px card margin, timer « 1 j / 7 m » (`XIGt+W`, `rhY1Rs`, `GBLpQ8`) |
| Discover | overall | 8.3 | — | 8.3 | — (card banner crop differs) |
| Nitro home | overall | 16.5 | — | 16.5 | greeting by hour (`Bonjour,` 5–16 h, `Bonsoir,` otherwise), hero padding 80/64, 2 or 3 columns from the container queries |
| Quests | overall | 40.4 | — | 40.4 | — (the client hero is a sponsored quest, Phoenix the Orbs promo) |
| Shop | overall | 23.2 | — | 23.2 | — (Dark Mode Friday sale, countdown and discounted prices missing) |

### Settings, server settings, store pages and popouts (same night, 1280×700)

User settings (one pass per page, opened and closed, no write): « content » is the page, « nav » the sidebar. The nav
scores mostly measure its scroll position: the driver scrolls the clicked entry into view, Phoenix reveals it
itself, so only the content column is a fair score.

| Page | Content | Nav | Overall |
|---|---|---|---|
| Compte | 1.0 | 2.4 | 1.2 |
| Données et confidentialité | 5.1 | 15.9 | 6.6 |
| Permissions de messagerie | 4.3 | 19.9 | 6.7 |
| Notifications | 2.8 | 14.1 | 4.6 |
| Nitro | 3.4 | 14.9 | 5.1 |
| Boost de serveur | 4.6 | 14.8 | 6.0 |
| Abonnements | 2.4 | 15.3 | 4.5 |
| Inventaire des cadeaux | 5.7 | 17.5 | 7.2 |
| Facturation | 1.9 | 18.0 | 4.6 |
| Voix & Vidéo | 7.2 | 17.2 | 8.3 |
| Apparence | 4.1 | 17.3 | 6.1 |
| Accessibilité | 5.0 | 17.5 | 6.8 |
| Système | 3.9 | 17.5 | 6.0 |
| Langue et heure | 1.7 | 18.5 | 4.6 |
| Jeux enregistrés | 3.3 | 18.3 | 5.7 |
| Confidentialité des activités | 3.9 | 23.3 | 7.0 |

Server settings of Aperture Sciences (read only; « Participation » never opened, « Membres » dropped because it
opens the member-safety route and sends `POST members-search`):

| Page | Content | Nav | Overall |
|---|---|---|---|
| Profil du serveur | 10.0 | 1.8 | 7.6 |
| Tag du serveur | 11.2 | 1.8 | 8.5 |
| Avantages de boost | 8.9 | 1.8 | 6.8 |
| Émoji | 2.8 | 1.9 | 2.6 |
| Autocollants | 13.8 | 1.9 | 10.3 |
| Soundboard | 4.3 | 1.9 | 3.6 |
| Rôles | 5.9 | 1.8 | 4.7 |
| Invitations | 4.9 | 1.8 | 4.0 |
| Accès | 17.1 | 1.8 | 12.7 (client content scrolled to « Profil ») |
| Intégrations | 5.9 | 9.4 | 6.9 |
| Configuration de Sécurité | 1.1 | 9.3 | 3.5 |
| Logs du serveur | — | — | — |
| Bannissements | 4.5 | 9.7 | 6.0 |
| Vue d'ensemble de la communauté | 9.1 | 8.8 | 9.0 |
| Processus d'accueil | 7.1 | 8.9 | 7.6 |
| Analyses de serveur | 8.9 | 9.4 | 9.1 |
| Présence sur internet | 4.2 | 10.0 | 5.9 |
| Modèle de serveur | 7.1 | 9.8 | 7.9 |

Store pages, recaptured at 12:05 with the client painting again (int-bin a4e9db40, overall mismatch): Découvrir 8.3 → 8.2,
Quêtes 40.4 → 35.7, Boutique 23.2 → 20.8, Nitro 16.5 → 14.6 (2 columns of 396.83 now, greeting and display name font
match; the gradient image now spreads behind the page, c2f99694).

Popouts: server menu 11.7 (header chevron turns up while open instead of a cross, fixed after the score), status /
account popout 17.6 (voice card server name now one line with an ellipsis; the « … » menu of the voice section is still
missing), poll creation modal: overall 5.2 (its box was not detected by the scorer).

### New views (28/11 12:40, client painting, int-bin 52d8bd63)

| View (1280×700) | Region | Score | Notes |
|---|---|---|---|
| Full profile (modal) | modal | 6.8 (overall 5.5) | — |
| Add a server (modal) | overall | 5.7 | — |
| Inbox, Mentions | popout | 8.1 (overall 6.4) | the inbox tab was put back on « Messages non lus » after the pass |
| Friends, Online | overall | 8.2 | online count and presences moved between capture and render (66 vs 63) |
| Friends, All | overall | 8.9 | same |
| Friends, Pending | overall | 7.1 | — |

Checked after the merge of 863eaa6b: the GIF picker body is black like `container_rfDq2B` (sampled pixels identical), the
emoji rail starts with the Nitro item and its separator and no longer lists the Project Phoenix server; both popout
scores stay at 12 % because the animated emojis and the « popular » order differ in time.

### Sources found for the store gaps (28/11 afternoon, not implemented yet)

- Shop sale (« Vendredi en mode sombre », -25 %): not in `GET /collectibles-shop`, `/collectibles-categories/v2`,
  `/collectibles-products/:sku` (prices `0` and `4` only, no tier `2` SALE), `/users/@me/collectibles-marketing` nor
  `/collectibles-collection-sequenced-offers` (empty). The client builds it from a promotion whose
  `tenantMetadata.collectibles.type` is `COLLECTION_SALE` with `rewardStatus === EARNED` and `skuIds` containing the SKU
  (module 664749 `c`), and the banner card from `rX` (chunk 4a63ee7d92c2ac58: `percentOff`, `endsAt`, `countdownStyle`,
  `showCountdownCta`; text `97Qza2` « Profite de -{percent} % sur les articles {collectionName} ! », countdown `z0`
  dd:hh:mm:ss). The promotion feed that carries it is still to be found.
- Nitro home art (live DOM): page gradient `https://discord.com/assets/3ebfa123a3805f56.svg` (881.67×984), flying
  Wumpus `…/a3e8e17987398023e2afd61ec5078a9bce18b2832f2f1775a1ba3c033ce13270.webm` (loop, 174.56²) with four PNG
  trinkets (bolt, hammer, key, star). Tenure card first: pill « Récompense » (12/16 semibold on white), badge
  `…/d0e033cda164abbfc02c39e70afcccb410954ceb63544178608eff0323092ccd.png` 276×230 (Silver), « Abonné depuis 17/08/26 »
  14/18, « Nitro Argent » 20/24 semibold, « 3 mois avant Nitro Or », button « Je veux y aller maintenant ».
- Emoji picker: the Project Phoenix server has one emoji, locked to role 1574895462320709685, which the account does not
  have; `EmojiStore.isUsable` (`0 === roles.length` or a member role in `emoji.roles`) hides it, so the server has no
  section. Phoenix now filters the same way (`discord_backend/expressions.rs` `usable`).
- « Montrer tous les salons »: `checked: !jN(guild)`, `jN = isOptInEnabled && (COMMUNITY || staff)`, opt-in = guild
  settings flag `OPT_IN_CHANNELS_ON` 16384 (module 395504, chunk a8dfb64ebcb89184). Phoenix now reads it.
- Profile voice section « … »: `contextMenu_Bu4Al8` 16×16 `MoreHorizontalIcon` xs, `margin-inline-end: 4px`, tooltip
  `UKOtz+`, header `space-between`.
- Profile popout message field: placeholder box 197.67 px in a 268 px field (`inner_2x8-Qn` pl 15, slate editor
  207.67, placeholder 10 px narrower).
- Invite modal not compared: opening it creates an invite (`POST /channels/:id/invites`).
- Solo call view not compared: the user was in a voice call all day.

### After-renders (28/11 14:15, int-bin 01a9b0fa / 5a45a5e6)

| View (1280×700) | Region | Before → After | Notes |
|---|---|---|---|
| Nitro home | overall | 16.5 → 14.6 → **9.4** | transparent libvpx Wumpus video + 4 trinkets + page gradient SVG + tenure reward card first (`9abcf299`) |
| Shop | overall | 23.2 → **21.4** | `RÉDUCTION DE 25 %` badges and struck prices live (`5a45a5e6`); Orbs pill in header clips on the right at 1280 |
| Quests | overall | 40.4 → 35.7 → 42.8 | DUNE sponsored quest hero + « Sponsorisée » badge (`2ea82f3a`); 42.8 % vs earlier client shot is the video frame offset |
| Server menu (`guilddrop`) | popout | 11.7 → **11.5** | header chevron points up while open, « Montrer tous les salons » checked |
| Account / voice popout (`status`) | popout | 17.6 → **15.6** | voice card « … » menu button + single-line server name with ellipsis |
| Member profile (`profile`) | popout | **17.0** | message input placeholder truncates cleanly before the emoji button |

### Still open, by visibility

- Shop: header Orbs pill no longer clips at 1280 px (`shop_page.rs` compact nav spacing + flexible search bar `120..256` px, `flex_none` on wishlist and Orbs pill); `COLLECTION_SALE` countdown banner (`container_arblTP`, 292 px, 22×32 digit tiles, 6×16 colons, `Dark`/`Light`/`GradientV1`/`Transparent` styles, `97Qza2` copy and `5GoOlx` CTA) implemented in `shop_page.rs` (`wt/shop-orbs`). Still open: promotion feed endpoint carrying live `COLLECTION_SALE` assignments and header search rotating suggestions (`collectibles_search_bar`, chunk 6b4565136c5e825f).
- Gift inventory: banner art now loads the client Lottie (`GiftCardBanner`, `96c9c7a8574bdf47c51fa14ce5ebdf8caf88bfbb71597a60a1d80d8973180649.lottiejson`, 282×188, `aspect-ratio 6/4`) via `media::lottie::image` with static gift SVG fallback, and bullet icons use `xxs` 10 px (`settings/gift.rs`, `wt/shop-orbs`).

### Zoom 150 / 200 % (28/11 17:10, int-bin d1dd79a5)

Client on Friends (`/channels/@me`, the only ack-free route it was on), emulated to 853×467 CSS px at dpr 2.25 (150 %)
and 640×350 at dpr 3 (200 %), then `clearDeviceMetricsOverride`. Phoenix `real_shots PHX_W/PHX_H/PHX_SCALE` same sizes.
The offscreen buffer of `real_shots` stops at 2× the CSS size (1706×934 and 1280×700 instead of 1919×1051 and 1920×1050),
so only the left part is compared, on the client shot cropped to the same box.

| View | Zoom | Overall | Notes |
|---|---|---|---|
| Friends, Online | 150 % | 13.2 | rail, mention / voice badges, header and tabs line up; the client shows a « Quête terminée » card over the DM list |
| Friends, Online | 200 % | 10.8 | same; header tabs cut by the window like the client |
| Test channel (Phoenix only) | 150 / 200 % | — | member list kept, channel list fixed, chat column squeezed: same rules as `responsive.md` (channel views cannot be opened in the client, ack) |

Fixed: in a friend row the display name did not shrink (no `min-width: 0`), so a long name pushed the server tag out of
the row. Client CSS `.userInfo_jPuNMC{display:flex;min-width:0}`, `.username_jPuNMC{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}`:
the name ellipsizes (« ! Peдyaн Ịاlاه 마다… ») and the tag stays (`home/friends.rs`, 53bddccf). ❌ still open: after the merge a ~27 CSS px gap sits between the ellipsis and the tag (client ~5 px); the ellipsized name box keeps more width than its text. Rail badges at 150 / 200 % now compared: ✅.
