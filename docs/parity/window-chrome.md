# Window chrome (frameless window, app top bar, window buttons)

CSS: ~/discord-phoenix-ref/current-client/css/300559.*.css. Dump: vanilla-guild-text (#13 bar, #46..#52 window buttons).

| element | CSS source | property | CSS value | our code | status |
|---|---|---|---|---|---|
| window | Electron `frame: false` | decorations | frameless | lib.rs window_options `WindowDecorations::Client` | ✅ (needs live check) |
| top bar | `bar_3zSH5d` | height | `--custom-app-top-bar-height` 32px | title_bar.rs render | ✅ |
| top bar | `bar_3zSH5d` | `-webkit-app-region` | drag | window_chrome::drag_region (move on drag, dblclick = maximize) | ✅ |
| bar buttons | `clickable_` / `winButtons_` | `-webkit-app-region` / pointer-events | no-drag / auto | `.occlude()` | ✅ |
| win buttons | `winButtons_3zSH5d` | gap | `--space-4` 4px | win_buttons | ✅ |
| divider | `winButtonsWithDivider_:before` | border / height / margin-inline | 1px `--border-subtle` / 32-12 / 4px | win_buttons | ✅ |
| button | `winButton_3zSH5d` | size | 32×32 | win_buttons btn | ✅ |
| glyph | dump #48/#50/#52 | box | 12×12 | glyph_box | ✅ |
| minimize glyph | dump #48 | shape | 10×1 rect at (1,6) | win-min-g | ✅ |
| maximize glyph | dump #50 | shape | 9×9 rect at 1.5, 1px stroke | win-max-g (10×10 border) | ✅ |
| close glyph | dump #52 | shape | 10×10 cross | assets `icons/window/close.svg` (own drawing) | ✅ |
| color | `winButtons_` | color | `--interactive-text-default` | fg | ✅ |
| min/max hover | `winButtonMinMax_:hover` | bg / color | `--interactive-background-hover` / `--interactive-text-hover` | hover + group_hover | ✅ |
| min/max active | `winButtonMinMax_:active` | bg / color | `--interactive-background-active` / `--interactive-text-active` | active + group_active | ✅ |
| close hover | `winButtonClose_:hover` | bg / color | `--control-critical-primary-background-default` / `-icon-default` | hover + group_hover | ✅ |
| resize edges | Chromium frameless Linux | inner border | 5px, corners 16px, ns/ew/nwse/nesw cursors | window_chrome::resize_handles | ✅ (no CSS; compositor resize) |
| maximized | — | resize strips | none when maximized/tiled/fullscreen | resize_handles | ✅ |
| Windows buttons | Electron `frame: false` on win32, `winButton_` | min / max-restore / close | native caption actions | `cfg!(windows)`: buttons always drawn, each one is a `WindowControlArea` (`HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE`), so gpui-pre-windows minimizes, toggles maximize and restore, and posts `WM_CLOSE` (tray rule in `on_window_should_close`); free bar = `WindowControlArea::Drag` (`HTCAPTION`). `Window::zoom_window` only maximizes on Windows, so it is not used there | ✅ code / ⚠ not run on Windows here (#14) |

State coverage: default, hover, active (min/max), close hover. No focus/tooltip on window buttons in the client.
| close button | client "Minimize to tray" (default on) | behaviour | hide to tray | title_bar win-close → minimize when tray is up; tray.rs SNI Open/Quit | ⚠ minimised, not hidden: see "Hide to tray on Wayland" below |

## System tray (`tray.rs`)

Reference: `discord_desktop_core` `systemTray` module (core.asar of app 1.0.171, read 2026-11-24) and the
web client's `SystemTrayManager` (strings `CK()`, states `C$()`).

| Item | Client | Phoenix | Status |
|---|---|---|---|
| Protocol | Electron `Tray` on Linux = StatusNotifierItem + dbusmenu | zbus SNI (`org.kde.StatusNotifierItem-<pid>-1`) + `com.canonical.dbusmenu` | ✅ |
| Left click | `click` → `onTrayClicked` (show the window) | Plasma's system tray (`org.kde.plasma.systemtray`, `libKF6StatusNotifierItem`) calls `ProvideXdgActivationToken(token)` right before `Activate`; `tray::activation` keeps the token (single use, 10 s) and sends `xdg_activation_v1.activate(token, wl_surface)` for the window on GPUI's own `wl_display` (guest `wayland-client` backend from the raw window handle), so KWin raises and unminimises the window; without a token (X11, menu item "Open", other trays) it falls back to `Window::activate_window` | ✅ code + unit test; ⚠ not clicked live (a test instance would put a second window and tray icon on the user's screen) |
| Tooltip | `setToolTip(appName)` | `ToolTip` = "Discord Phoenix" | ✅ |
| Menu | `[Open {app}]`, then, connected only, `Rendre muet` / `Mettre en sourdine` (checkboxes) and `Ouvrir les paramètres vocaux et vidéo`, then `Rechercher les mises à jour…` / `Remerciements`, then `Quitter {app}`; a separator before each non-empty group | same order and labels (fr/en from the client's strings), checkmarks follow the panel mute/deafen, Voice settings opens Settings > Voix & vidéo, Remerciements opens `discord.com/acknowledgements` | ✅ / ➖ no "Check for updates" (Phoenix has no updater) |
| Icons | `tray`, `tray-unread`, `tray-connected`, `tray-speaking`, `tray-muted`, `tray-deafened` from `app/images/systemtray/linux`; priority connected → deafened > muted > speaking > connected, else unread, else default | the same PNGs loaded at runtime from the local Discord install (never committed), padded app icon (24 px) as fallback. `IconPixmap` carries one 24 px image per state, `linux/<name>.png`, exactly what Electron sends (checked live 27/11 with `busctl get-property` on Discord's `org.freedesktop.StatusNotifierItem-<pid>-1`: `a(iiay)` with a single 24×24 entry, no `IconName`). The earlier multi-size list (win32 16 + `@1.25x…@5x`) made Plasma 6 pick the 22 px `@1.4x` image and shrink it into its 16 px slot (test SNI items on the user's panel, 27/11: the 11-size list renders pixel for pixel like a 22-only item, with faint eyes, while a 24-only item renders like Discord), so the blur came from that non-integer downscale (`tray::tray_file`, `client_pixmaps`); state polled every 500 ms, `NewIcon` / `LayoutUpdated` signals | ✅ unread = `!disableUnreadBadge && (hasAnyUnread || totalMentions > 0 || pending requests > 0)`. Server state (web `C$`/`CY`, core.asar `getTrayStates`): deafened = `selfDeaf \|\| serverDeaf`, muted = `selfMute \|\| serverMute \|\| suppress`; the menu's mute row becomes « Interdit » (`FJSZVM`) when suppressed or « Serveur muet » (`uLddbQ`) when server muted, the deafen row « Mis en sourdine sur le serveur » (`btxSdB`), both checked and disabled (`TrayState::server_mute/suppress/server_deaf`, `mute_item`, `deafen_item`, test `server_mute_and_suppress_drive_the_icon_and_lock_the_rows`) |
| Close hint balloon | `displayBalloon` (Windows only) | — | ➖ |

Tests run on a separate process only: the user's Phoenix and windows are never closed or minimised.

## Hide to tray on Wayland

The client hides its window (`BrowserWindow.hide()`) when "Minimize to tray" is on. Phoenix minimises
instead, on purpose:

* an `xdg_toplevel` can only disappear by being unmapped (null buffer + commit), and mapping it again
  needs a fresh initial commit / configure cycle;
* GPUI 0.3.12 has no hide / show for a window (`PlatformWindow` has `minimize`, `activate` and
  `remove`): an unmap done behind its back would be undone by the next frame, or break the configure
  handshake;
* closing the window and opening a new one on Open would rebuild the root view's window state
  (focus handles, text inputs, IME, scroll positions, subscriptions) and needs GPUI work first.

So the close button minimises when the tray is up (KWin keeps the task bar entry), and the tray Open
action raises it with the token above. Revisit if GPUI gains `hide`/`show` for Wayland windows.

## Floating PiP, Call Popout & Compact Window Mode

- **Floating Voice/Video PiP** (`ui/pip.rs`): shown when connected to a voice channel while viewing another channel (and not popped out), only for a stream the user is watching: live 2026-11-25 over CDP on another route, `[class*=pictureInPicture_]` has 0 children for a voice-only call, an unwatched stream and the camera on. 320×180 16:9 card, draggable into all 4 corners with snap-to-corner and viewport clamping, showing active stream/camera/speaker, hover controls (mic, deafen, disconnect, return to call, popout).
- **PiP video**: the card draws the watched stream from the same frame cache as the call tiles (`call_view::stream_image`), sends media sink wants with `pip: true` (no focused 100 layer) and its 320x180 size × zoom × scale factor as the Go Live view size; replaced frames are dropped from the sprite atlas of every window (`call_view::release_video_images`).
- **Multi-window Call Popout** (`ui/call_popout.rs`): `call-popout` button in `call_view.rs` and `pip.rs` opens a secondary frameless GPUI window (`cx.open_window`) observing `PhoenixApp` for live state sync (tiles, video/stream, mute/deafen/disconnect), with clean return to the main window on close.
- **Narrow windows**: the custom compact mode (member list hidden under 940 px, channel drawer under 720 px) was removed: the 2026 client has neither. The real thresholds are in `responsive.md` (section "Width thresholds from the client CSS and JS"). Window minimum 940×500 and default 1280×720 from `discord_desktop_core` `core.asar` (`MIN_WIDTH=settings.get("MIN_WIDTH",940)`, `MIN_HEIGHT … 500`, `width:1280,height:720`).
- **Window Shortcuts** (`shortcuts.rs`): `F11` toggles fullscreen (`ToggleFullscreen`), `Ctrl+`/`Ctrl-`/`Ctrl+0` step page zoom (50–200 %).

## Task bar icon and the rail's media badge (2026-11-25)

The reported capture (red Phoenix icon, green "screen" circle top right, red "1" bottom right) is not drawn by the
desktop app on the task bar. Its colours are exactly two client tokens: green (61,158,96) = `--green-new-40`
(`--green-360`, `--status-positive`) and red (210,45,57) = `--red-new-50`. It is the server rail's guild icon
(the Project Phoenix server's icon) with `upperBadge_` and `lowerBadge_`.

What the desktop app does with the OS icon, read in `core.asar` and `app.asar` of app 1.0.171 and the web bundle:

| Item | Client | Phoenix | Status |
|---|---|---|---|
| Badge count | web `setBadge(n)`: macOS `dock.setBadge`, Windows `APP_BADGE_SET` → `win.setOverlayIcon(badge-N.ico)` (`app/images/badges/badge-1..11.ico`, 11 = unread dot), Linux `app.setBadgeCount(n >= 0 ? n : 0)` | Unity `com.canonical.Unity.LauncherEntry.Update` with `count` / `count-visible` (tray.rs `launcher_props`), the same number | ✅ |
| Overlay icons on Linux | none: `setOverlayIcon` is Windows only, no `setIcon` call in core.asar | none | ✅ |
| Window icon | static | static (GPUI 0.3.12 exposes no window icon API and no `xdg_toplevel` handle) | ➖ |
| App icon (desktop entry, packages, .ico, .icns, window icon) | Discord's launcher icon in the user's theme (MacTahoeLiquidKde `com.discordapp.Discord.png`, one 256 px image scaled for every size): opaque squircle 87.5 % of the canvas, centred, 1/16 margin per side (16 → 1 px, 22 → 1.375, 24 → 1.5, 32 → 2, 48 → 3, 64 → 4, 128 → 8, 256 → 16, 512 → 32; measured on the alpha ≥ 128 box) | the Phoenix logo (`assets/icon.png`, body 505 px on 512) scaled into the same 87.5 % box at 16, 22, 24, 32, 48, 64, 128, 256, 512 (8× supersampled, premultiplied): `packaging/icons/hicolor/*`, `discord-phoenix.png` (512, also the window icon and the tray fallback), `.ico` (16 to 256), `.icns` (ic07 to ic14). The login screen keeps the full-bleed `icon.png` | ✅ same size as Discord in the Plasma dock |
| KDE Plasma task manager | draws the count badge from LauncherEntry and its own audio stream indicator top right (`AudioStream.qml`, `audio-volume-high-symbolic`) when the process has a playing PulseAudio stream; the count badge moves down then (`TaskBadgeOverlay.qml` `shiftBadgeDown`) | same, KDE-side | ✅ (KDE behaviour, not app code) |

### Rail media badge (`server_bar.rs` `guild_media` / `media_state`)

Source: web bundle `GuildMediaStateStore.getGuildMediaState` and the badge factory (`oi`) next to `upperBadge`.

| Rule | Client | Phoenix | Status |
|---|---|---|---|
| Icon priority | event → Calendar, live stage → Stage, screenshare → `ScreenIcon`, video → `VideoIcon`, audio → `VoiceNormalIcon`, activity → Apps | active event → `calendar`, live stage → `stage`, then screenshare → `golive/screen`, video → `video-call`, audio → `speaker` (`N` of module 771640) | ✅ event and stage: `guild_live` keeps the stage instances and the active scheduled events (status 2, `entity_type` ≠ 0, channel visible or none, `wX` of 508654) from GUILD_CREATE and the `STAGE_INSTANCE_*` / `GUILD_SCHEDULED_EVENT_*` dispatches; connected: stage = your channel is a stage, event = an active event in your channel; muted guild: nothing unless connected (`D`); test `events_and_stages_come_first_like_the_badge_factory`. ✅ activity (`apps`, last): `guild_live` keeps the embedded activity instances like `EmbeddedActivitiesStore` (`activity_instances` of GUILD_CREATE and READY_SUPPLEMENTAL guilds, `EMBEDDED_ACTIVITY_UPDATE_V2` upserted by `location.id` + `application_id`, removed when `participants` is empty); badge (`getGuildMediaState`): connected = any instance in the guild, else not muted and an instance whose channel is vocal (`gV`: voice, stage, threads) or launchable (`pE`: text/voice at top level with `USE_EMBEDDED_ACTIVITIES`), instances with a blocked or ignored participant skipped (`PH`); tests `embedded_activities_follow_their_participants`, `activities_come_last_and_skip_blocked_players`. ⚠ not seen live (needs someone else running an Activity). ➖ the suffixes (`dHvJ2p` « Événement programmé », `OO7ndG`, `wsHMZ7`, `BrLCS0`, `jPBhKy`, `Y3Gii5`) only go into the icon's `aria-label` (`eo` of the guild item), not the visible tooltip, and GPUI has no accessibility tree |
| Colour | `iconBadge_` `--icon-overlay-dark`, `isCurrentUserConnected_` `--status-positive` | same tokens | ✅ |
| Not connected to the guild | audio: any voice state of a non-blocked, non-ignored user in a visible, non-AFK, non-stage, non-muted channel; video: same with camera on; screenshare: any application stream of a non-blocked owner in a non-muted channel (one streamer is enough) | `Presence` list from `voice_users` + `voice_panel::flags` | ✅ unit tests `media_tests` |
| Muted guild | no badge unless the user is connected there | same | ✅ |
| Connected guild | audio on, video = the voice channel has a camera on, screenshare = the current user's own stream (`getActiveStreamForUser(me, guild)`), green | same, own stream = `go_live::live` or own `self_stream` | ✅ |
| Folder (collapsed) | only the selected voice guild: audio = !video, video, own screenshare, green | `folder_media` | ✅ |

- ✅ (28/11) Main window opens maximized on the primary display (`WindowBounds::Maximized`, `display_id` = `cx.primary_display()`); un-maximizing restores 1280×720 (clamped to the visible area) centered on that display. No saved geometry overrides it. On Wayland gpui sends `set_maximized` before the first buffer, so no small-then-maximized flash. Call popout, overlay and splash unchanged.
