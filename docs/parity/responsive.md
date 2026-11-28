# Responsive layout (narrow windows, zoom 50–200 %)

Reference: the live 2026 client over CDP, 2026-11-24, theme Onyx, cozy, font 16, zoom 125 %
(`devicePixelRatio` 1.25), guild text channel without a topic, member list shown, channel list
dragged to 345 px. The width was changed only through `Emulation.setDeviceMetricsOverride`
(the user's window was never moved or resized), then `Emulation.clearDeviceMetricsOverride`
restored 1536×788. All values below are CSS px (`innerWidth`).

## How zoom works

Discord's zoom is Chromium page zoom: a window of W device-independent px is `W / zoom` CSS px
wide and every rule below applies to that CSS width. Phoenix does the same (`set_page_zoom`, see
`docs/gpui-patches.md`): layout runs in CSS px and the thresholds below are written in CSS px,
so they hold at every zoom level. A 1700 px window at 150 % is a 1133 CSS px layout; at 200 %
it is 850 CSS px.

Offscreen check without a zoom setting: `PHX_W=<css width> PHX_SCALE=<dpr × zoom>` on
`real_shots` (for example `PHX_W=1130 PHX_SCALE=1.875` is 150 % at dpr 1.25), or
`theme_shots out.png dark 150|200` on the demo data.

## Thresholds

| Area | Live behaviour | Phoenix | State |
|---|---|---|---|
| Member list | Never hidden by width. Stays 268 px down to `innerWidth` 480 even when the chat column reaches 0 px; only the toolbar button hides it | `ui/chat.rs` shows it whenever `show_members` | ✅ (no auto-hide, same as live) |
| Channel list + rail | Fixed (416 px here, the dragged width); never shrinks with the window | unchanged | ✅ |
| Header search | `clamp(64, header content width − 320, 244)`; content width = header border box − 28 (padding 20 + 8). Header 983 → 244, 583 → 235, 543 → 195, 483 → 135, 433 → 85, 383 → 64. Below that it is squeezed further (38 px at header 223) | `header_search_w` from a width probe on the header | ✅ |
| Header toolbar buttons | Threads, notifications, pins, member list: all four stay at every width (32 px, gap 10, padding-left 10, search margin 4). No overflow menu | unchanged | ✅ |
| Channel title | `titleWrapper_` is flex-shrink 0: the title keeps its full width and `children_` (flex 1, overflow hidden) clips it once the search is at 64 px (children_ 138 → 109 → 9). No ellipsis on the title | title `flex_shrink(0)` except thread titles (flex 0 1 auto) | ✅ (was shrinking to "Di") |
| Composer buttons | `channelTextArea_` ≥ 450 px: gift, GIF, sticker, emoji, apps. Below (449.8 measured): gift, emoji, apps. Nothing else hides, even at 2 px | `full_buttons` from a width probe on the text area | ✅ |
| Server rail badges | Icon 40 px; `upperBadge_` 16 px at (24, 0) (icon choice: window-chrome.md "Rail media badge"), `lowerBadge_` 16 px at (24, 24), cut-out rings; the two never touch | same boxes at 100 % (pixel check on `theme_shots` at dpr 2) | ✅ (layout); rasterisation at 150/200 % not compared against a live capture at that zoom |

Offscreen check after the merge (7b62021, `theme_shots` demo data, 1280×800 at dpr 2): at 150 %
(853 CSS px) the title stays whole, the topic takes the remaining room, the search is about 175 px (read off the image) and
the composer shows gift, emoji, apps; at 200 % (640 CSS px) the search is 64 px and the title is
clipped by `children_` exactly like the client at that width.

Headers with a topic, DM, group DM and forum headers are now covered in `chat.md` (section
"DM, group DM and forum headers"); density and font size are below.

## Member list (read-only checks, 2026-11-24)

Same guild (44 members, under the lazy list's first 100-row range), live list read with a 2080 px
tall emulated viewport so the whole list is in the DOM, Phoenix with `real_shots PHX_H=2080`.

| Check | Live | Phoenix | State |
|---|---|---|---|
| Groups and counts | hoisted roles in server order, then "Hors ligne — 25" | same four groups, same counts and order (rows come from GUILD_MEMBER_LIST_UPDATE as sent) | ✅ |
| Offline group | present below the role groups for this guild size | rendered from the server's `offline` group | ✅ |
| Owner crown | no `ownerIcon_` when the owner sits in a hoisted role group; dumps (`vanilla-threads-lrer`, `vanilla-search-popout-2`) show it in "Hors ligne" | crown only outside hoisted role sections (wt/members26) | ✅ (2 data points) |
| List per channel | Live 2026-11-24, guild PGE (44 members), `💬┆discussion`: Activité — 1, BOT — 1, PGE4 — 16/18, one more role group, Hors ligne. The gateway's list `id` is per set of VIEW_CHANNEL overwrites: murmur3-32 (seed 0) of the sorted `allow:<id>` / `deny:<id>` entries joined by `,`, or `everyone` when none; checked on four channels (1093928432, 2860088392, 1037889033, `everyone`) | Was one list per guild: the staff-only first channel opened at start had its SYNC land after the open channel's, so discussion showed the 🔑moderation list (BOT 1, PGE4 3, Hors ligne 2) and no Activité section. Now lists are kept per list id and only the open channel's reaches the UI; op 37 is re-sent on each channel switch | ✅ (wt/members2) |
| Range loading | — | [0, 99] on open, then the chunk in view and the next one (op 37) with placeholder rows | ➖ needs a guild with more than 100 members open in the live client: opening any guild other than the Aperture Sciences test channels acks its channel, which the brief forbids |
| "Afficher plus" | not found in this guild's list | — | ➖ only shows in a large guild's list, same navigation limit as the row above |
| Activity section | Live 2026-11-24 (wt/sweep): the section only exists when at least one card is shown, and its header counts the cards shown. Guild PGE: feed with 6 matching entries, 5 by non-members → "Activité — 1", one card. Aperture Sciences: no card → no section | Phoenix counted the entries whose author was not loaded (pending op 8); non-members never resolve, so it kept "Activité — 6" (and "— 7" on Aperture). Now count = cards | ✅ (wt/sweep) |
| Activity card timer | entry with trait 3 `is_live: true`: green timer = now − `started_at` ("12:08" measured to the second), even when the author's presence has no matching start | timer from `started_at` while `is_live` and `expires_at` is in the future; presence start as fallback; "x ago" otherwise | ✅ (wt/sweep) |

## Density and font size (live, 2026-11-24)

Method: one synchronous CDP `Runtime.evaluate` on the live client (1536×840 CSS px, zoom 125 %,
guild text channel already open, nothing clicked). It swapped the `<html>` classes the client sets
itself (`density-compact|default|cozy`, `font-size-N`, plus `a11y-font-scaled-down` under 16px
and `a11y-font-scaled-up` above, as the client's `fontScaleClass` does) and the root
`font-size: N/16 %`, forced a layout, read boxes and computed styles, then restored the exact class
string and style attribute (checked: `restored: true`, same route and active element). No frame was
painted in between and the Settings were never opened. Configs: cozy/default/compact at 16px,
cozy at 12/14/18/24px, compact at 12/24px.

Zoom is not a CSS class: it is Chromium page zoom, so a zoom level is just another CSS width (see
"How zoom works" above); the font size is what changes the layout at a given CSS width.

| Area | compact / default / cozy (16px) | font 12 / 14 / 18 / 24px (cozy) | Phoenix | State |
|---|---|---|---|---|
| `--space-xxs…xxl` | 4 6 10 12 16 20 26 / 4 8 12 16 20 24 32 / 4 10 16 20 24 30 40 | unchanged | `theme::Density::scale` | ✅ |
| Header section | padding-left 12 / 16 / 20 (`--space-md`), height 49 | unchanged (header is px, not rem) | chat.rs `header` | ✅ (was 20 fixed) |
| Header toolbar | gap and padding-left 6 / 8 / 10 (`--space-xs`); buttons 32, icons 20 | unchanged | chat.rs `header` | ✅ (was 10 fixed) |
| Header search | 244 × 28 / 32 / 32 | unchanged | chat.rs `header` | ✅ (was 32) |
| Guild rail | 64 / 72 / 80 (40 + 2 × `--space-md`); the channel list widens by the same amount (rail + list stays 415.8) | unchanged | `m::server_bar_w`, `m::sidebar_extra_w`, server_bar.rs gap `--space-xs` | ✅ (was 80 fixed) |
| Message row (group start) | padding 2 20 2 64 / 2 24 2 72 / 2 30 2 80 (`--space-xl`, 40 + 2 × `--space-md`), margin-top 17 | padding-block .125rem (1.5 / 1.75 / 2.25 / 3), margin-top 1.0625rem (12.75 / 14.875 / 19.125 / 25.5); padding-left 54 / 63 at 12 / 14 (`a11y-font-scaled-down` 4.5rem), 80 at 18 / 24 | chat.rs `message`, `msg_left` | ✅ (gap, padding and the scaled-down indent were fixed px) |
| Avatar | 40, margin-top 2 | 30 / 35 at 12 / 14 (2.5rem, left 1rem), 40 at 18 / 24 (margin-top 4px − .125rem) | chat.rs `msg_avatar` | ✅ (was always 40 at 20px) |
| Username / content | 16 / 22 | 1rem / 1.375rem (12/16.5, 14/19.25, 18/24.75, 24/33) | `fz`; text with emoji (`rich_with_emoji_scaled`) | ✅ (text carrying an emoji stayed at 16px: seen on a `real_shots` render at PHX_FONT=24, fixed) |
| Cozy timestamp | 12 / 22 | .75rem (9, 10.5, 13.5, 18) | chat.rs `message` | ✅ (was 12 fixed) |
| Composer | text area 56 tall, margin-bottom 6 / 8 / 10 (`--space-xs`), form padding-inline `--space-xs`; attach 32 (margin 0 10 0 −16), buttons 32 | slate 1rem / 1.375rem, padding-block (56 − 1.375rem) / 2 = 19.75 / 18.375 / 15.625 / 11.5; box stays 56 | composer.rs `text_pad_y` | ✅ (text was 16px fixed) |
| Member list | width 256 / 264 / 268; group header padding 16 4 4 12 / 20 4 4 16 / 24 4 4 20 (`--space-lg`, `--space-md`), 14/18 | unchanged (member list is px) | `m::member_list_w`, members.rs | ✅ (was 268 / 24 4 4 20 fixed). The 1px `border-inline-start` of `container_86es0b` sits outside that width: the container is 269 wide (dumps vanilla-guild-text / chat-busy: container x 1011 w 269, membersWrap x 1012 w 268), so the panel is `member_list_w + 1`. Correction (wt/text-rest): the computed `border-left` is `0.666667px` at DPR 1.5 (Chromium floors border widths to whole device pixels), so the container is 268.667, not 269 (screenshot: border pixel at x 1011.33). GPUI snaps borders the same way in layout (`round_stroke_to_device_pixel`), so members.rs and group_dm.rs now give the 268 to an inner box and keep the border outside it |
| Member row | 44 tall, padding 1 0, ml 8, width list − 16; child padding-left `--space-xs` | unchanged | members.rs | ✅ |
| Channel list rows | category row padding-top 8 compact, 16 otherwise; channel name 14/18 | unchanged | sidebar.rs, `theme::space::{channels_name_lh, spine_left, category_before}` | ✅ (wt/sounds) from the cached CSS: `.density-compact|default|cozy{--channels-name-line-height:20/24/28px; --channels-spine-offset-left:20/24/28px}`, `:root{--custom-category-channel-space-before-category:16px}` and `.density-compact{…:var(--space-8)}` on `containerDefault_yOdhm_`. Channel rows are name line-height + 8 (28/32/36), connected voice rows + 24, thread rows + 8 with the spine at the offset, thread inset `--space-xs` |
| Compact message display | CSS (`738015`, `419070`): `--custom-message-margin-compact-indent: 5rem`, `--custom-message-padding-vertical-container-compact: .125rem`, `.compact_ .timestamp_` `.6875rem` / `margin-inline-end: .25rem`, `.compact_ .avatar_` inline `1em × 1em` (`margin-inline: .1em .25em`) | chat.rs compact branch: `5rem` indent, `.6875rem` timestamp, `.125rem` padding, and inline `1em` avatar (`ml .1em`, `mr .25em`) all follow `fz` | ✅ (CSS verified; inline `1em` compact avatar aligned, was 40px absolute) |

Offscreen check: `real_shots` now reads the density and font size from the client's `<html>` classes,
or `PHX_DENSITY=compact|default|cozy` and `PHX_FONT=12..24`. `PHX_COMPACT=1` is compact message
display only: it used to be turned on by `density-compact`.

## Width thresholds from the client CSS and JS (2026-11-25)

Sources: the client's cached CSS (every `@media` rule with a width or height query, 867 files) and
JS bundles from the local cache. All values are CSS px (`innerWidth`, after zoom), so a 1700 px
window at 150 % is 1133 CSS px.

| Area | Client | Phoenix | State |
|---|---|---|---|
| DM profile panel | `9fbf85fbb6aa2c59.js` `l1()`: `useState(window.innerWidth>=1132)` + resize listener; below it the header button is `disabled` with tooltip `YneDgF` "Afficher le profil d'utilisateur (Indisponible)" | `title_bar::dm_profile_fits`, `dm_profile_panel::render`, disabled `states::header_button_disabled` in the DM header | ✅ |
| Friends: Active Now column | `599618.*.css` `@media (max-width:1200px){.nowPlayingColumn_zrUwGA{display:none}}`; column `flex:0 1 30%; min-width:360px; max-width:420px` | `title_bar::active_now_shown`, `active_now_w` | ✅ |
| Friends: invite toolbar (new group DM) | same file, `@media (max-width:940px){.inviteToolbar_zrUwGA{display:none}}` | `title_bar::friends_invite_shown` | ✅ |
| Title bar title | `761444.*.css` `.title_LnjVAX{display:inline-flex;gap:var(--space-8);max-width:50vw}`; the 16 px guild icon stays, the text is clipped with an ellipsis | `title_bar::title_max_w` = 50 % of the CSS width; icon `flex_none`, text `min_w_0` + ellipsis | ✅ |
| Member list | no width media query in the chat layout (the only `membersWrap_86es0b` query is `max-width:485px`, below the 940 px window minimum at 100 %) | always shown when toggled on | ✅ (the old `< 940 px` auto-hide was removed) |
| Channel drawer | none in the client | removed (was `< 720 px`) | ✅ |
| Window minimum | `core.asar`: 940 × 500 DIP, default 1280 × 720 | `lib.rs window_options` | ✅ |
| Call PiP | live 2026-11-25: `[class*=pictureInPicture_]` stays empty for a voice-only call while another member is live (EN DIRECT) but not watched, and with the camera on ("Vidéo connectée") | `pip::active` needs a stream in `voice.watching` | ✅ (the voice card covered the composer at every width); a watched stream was not checked live |
| Title vs title-bar buttons | `Emulation.setDeviceMetricsOverride` 588 / 783 DIP at zoom 125 % (470 / 626 CSS px), then cleared: `.title_LnjVAX` 118–353 / 157–470, `trailing_3zSH5d` 282–470 / 438–626 with `z-index:1`, so the client's own title runs under the inbox and help buttons below about 750 CSS px | same geometry (title centred at 50vw under the trailing group) | ✅ same as the client |
| Message header | name, tags and time are inline in `.header_`, so the time wraps to the next line | header row `flex_wrap`, time 22 px tall | ✅ (the time was clipped at 752 CSS px) |

Unit tests: `ui::title_bar::tests` cover each threshold on both sides and at the test widths
(940, 1100, 1280, 1700 and 1920 px at 100, 125, 150 and 200 %).

"Programmer un sondage" (`rlf0tb`) in the poll modal: done from the lazy modal chunk fetched from the
assets CDN (see the poll creation row in chat.md).

## Thread sidebar and search in narrow windows (2026-11-28)

Source: the client's JS from the local cache, read only (no CDP action).

| Area | Client | Phoenix | State |
|---|---|---|---|
| Search vs thread | `ChannelSectionStore.getSection` (`web.b63d846b329d6470.js`) returns `SEARCH` while a search is shown, before `SIDEBAR_CHAT`; `renderThreadSidebar` (`9b3cd218fe829ac6.js`) only renders for `SIDEBAR_CHAT`. The thread stays in `sidebars[channelId]` and comes back when the search is cleared (`SEARCH_MESSAGES_CLEAR`); the member list is hidden the same way | `ui/chat.rs`: no thread panel while `search::results_shown`, the thread state is kept, Escape closes the search and the thread is back | ✅ (was squeezing chat and thread to a few px at 940) |
| Opening a thread during a search | `SIDEBAR_VIEW_CHANNEL` sets the search flag to false | `open_thread` closes the shown results | ✅ |
| Search results width | `.searchResultsWrap_QHVbp3{flex:0 0 auto;width:418px;border-inline-start:1px}` | fixed 420, no longer shrunk with the window | ✅ |
| Thread sidebar width | `ChannelChatResizableSidebar` (`1c44b4290d7ddc85.js`): `minWidth` 450, width `clamp(saved ?? 450, 450, maxWidth)`, `maxWidth = width − 240 − 450 + 375` with `width` the channel area (`456412` resize HOC), so `width − 315` | 450 (no resize handle yet) | ✅ |
| Floating thread | `maxWidth <= minWidth` (`743898`) → `floating_FrbQJp`: absolute at the right edge over the chat, `box-shadow:0 8px 40px var(--opacity-black-52)`, no spacer, so the chat keeps its full width; this happens when the channel area is ≤ 765 px (a 940 px window at 100 % with a 240 px channel list) | `thread_sidebar_layout`: same threshold, panel absolute over the chat with the same shadow | ✅ |
