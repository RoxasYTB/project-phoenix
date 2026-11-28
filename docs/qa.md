# Input QA

Static renders prove a view looks right, not that it reacts. The input QA runs
the offscreen `real_shots` window on the real account (read-only backend, no
acks) and feeds it the same `PlatformInput` events the Linux platform sends:
wheel notches as `ScrollDelta::Lines` (3 lines a notch, like Wayland
`AxisValue120`), touchpad swipes as a burst of `ScrollDelta::Pixels`, mouse
down / up with a click count, moves with a pressed button for drags, and
keystrokes through `window.dispatch_keystroke` (so key bindings and the text
input handler both see them). Between steps it reads app state and compares.

```sh
cd /mnt/windata/project-phoenix/compare/qa-input
PHX_AGENT=1 PHX_NO_SCROLL=1 PHX_MEMBERS=true PHX_IN_VOICE=false PHX_FOLDERS=0 \
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 PHX_GUILD=1369382919948210248 \
PHX_CHANNEL=1403447573389381652 PHX_SCRIPT=~/discord-phoenix/scripts/qa/basic_input.qa \
flock /tmp/phoenix-render.lock phoenix-nice ~/discord-phoenix/int-bin/real_shots qa.png
```

## Writes from agent sessions

With `PHX_AGENT=1`, every REST call that is not a GET goes through
`phoenix_api::rest::agent_scope` before it leaves the process (examples and the
full app alike). The allow list is closed:

- #phoenix (`1576039645127385188`): new messages, edits and deletes of a message,
  reactions, pins, polls, typing, attachment slots, threads created from it, and a
  PATCH of the channel itself that only sets `rate_limit_per_user` and/or `topic`.
- threads that this process created in #phoenix: messages, join, archive/lock.
- #général (`1403447573389381652`): new messages and attachment slots only.
- the test voice channel: `send-soundboard-sound`, plus the stream preview of a
  stream in that channel.
- interactions and scheduled messages whose `channel_id` is #phoenix.
- the POST routes that only read (member search, message search tabs).

Everything else is refused with `write blocked`, in particular `DELETE` or any
other PATCH of `/channels/{id}` itself, every guild, role, member, webhook,
invite and permission route, and every `/users/@me` setting. `real_shots`
refuses to start with `PHX_WRITE`, `PHX_SEND` or `PHX_VOICE` unless
`PHX_AGENT=1` is set. Requests to a loopback base (unit tests) are not scoped.

Trap channels (a « honeypot » that mutes whoever posts in it) are blocked for
every session, the user's included: `phoenix_api::trap` refuses any non-GET
request whose path or `channel_id` points at one (messages, typing, acks,
reactions, everything). They are recognised by id (Aperture's #honeypot,
`1507838362562859119`) and by a name or topic containing "honeypot", noted when
the channel enters the cache, threads of a trap included. Opening a server never
picks a trap as its default channel, and an agent session that tries to open one
stops the process (exit 70). `real_shots` stops with an error when the requested
channel is not on the account instead of falling back to another one, and agent
sessions never send acks.

The test channel #phoenix was deleted on 27/11 at 19:35 from the account. No
agent process could send that request at that time (the only `real_shots` run
was a read-only one still loading), but nothing in Phoenix itself refused it
either; this scope closes that. It was recreated on 28/11 with the same name, category and
private overwrites as the test voice channel: `〢💬・phoenix`, id `1576039645127385188`. It starts
empty, so the scroll steps run in #général (read-only) and the video step uses a test clip
uploaded to the new channel.

Without `PHX_NO_SCROLL`, `PHX_MEMBERS` and `PHX_IN_VOICE`, `real_shots` copies the
sidebar scroll, the member list toggle and the voice panel from the official client
over CDP. The 27/11 runs inherited a hidden member list that way (`members item #2 is
not laid out`, `members_y` stuck at 0) and a scrolled sidebar (channel click on the
wrong row), so the scenario pins them. `PHX_FOLDERS=0` closes every rail folder, so the
guild step opens the second folder (40,173) and clicks its first server (40,229).

#phoenix gets new test messages all the time, so the video steps do not click a
fixed point: `select` opens #phoenix and `jump <message id>` scrolls to the test video message (`1576040049177268324`, a generated 12 s test pattern) the way a reply
or search jump does, `@video` is the bounds of the video player painted last
(`region video @video` hashes its pixels, `@video.br+-16,-16` is the fullscreen
button), and `action bottom` goes back to the present afterwards.

The offscreen window runs on GPUI's test dispatcher, whose timers only fire when its
clock moves. `wait`, `until` and every step that drives frames advance that clock by
the real time spent, so timer-driven code (the video ticker, hover delays, springs
scheduled with `timer`) runs as on the desktop. Before 27/11 it did not: the video
started (`video_t=0.00`) but its ticker never ran between screenshots, which only
moved because each shot advances the clock by 4 s.

The attachment player is the legacy one unless `PHOENIX_DVP=1`: Space plays or
pauses, ←/→ seek 5 s, digits jump to a tenth, fullscreen is the bar button and Esc
leaves it. `l`, `j`, `k`, `f` and `m` exist only in the DVP player, so the scenario
uses `key right` and clicks the fullscreen button.

A bottom-aligned list that sits at its end anchors past its last item, and GPUI's
`ListState::bounds_for_item` then returns `None` for every row (`chat item #108 is
not laid out`). Point at the newest message from the list edge (`@chat.b+0,-60`)
instead of `@chat#-2` while the chat is at the bottom.

Each step drains the offscreen window's task queue with `input_script::settle`, which stops after
1.5 s of queued work and prints `QA note settle stopped ...`. Before 27/11 it was a plain
`run_until_parked`: the GIF picker's animated previews kept queueing decodes and repaints, so the
mouse-up of the GIF button never returned (no output for 15 min). `real_shots`' own pump uses the
same budget.

`text` keeps everything after the first space as typed (trailing spaces and `;` included), and
`@chat#-1.tl` (item then corner) is parsed as item `-1`, corner `tl`.

Every check prints `QA OK [step] ...` or `QA KO [step] ...`, then
`QA summary ok=N ko=M` and the list of shots (`qa-<name>.png`).
`PHX_SCRIPT_STRICT=1` turns a failed check into a failed run.

A `QA KO` is always one line: multi-line errors are folded with spaces (`diagnostics::one_line`). wgpu validation errors print `Validation Error` first and the actual cause (`Caused by: In Queue::write_texture ...`) on the following lines, so `grep "QA KO" log.txt` used to keep only the useless first line. This is how the cause of the inbox scroll failure at 1010 px (28/11 morning, real account, `PHX_SCALE=1.5`, 1280x700) was lost; it no longer reproduces on d1dd79a5 with the same script (wheel steps of 10 px to 1100 px), on the real account or with the demo data, which has no unread cards.

In the app the same GPU error does not crash: the renderer skips the frame, clears the atlas and its intermediate textures after 6 failed frames in a row and tries again (`WgpuRenderer::draw`), so a persistent error shows as a frozen frame with `GPU error during frame` in the log, never as a panic.

The mock counterpart, `ui/input_qa_tests.rs`, sends the same kind of events to a
`VisualTestContext` window on demo data and runs with `cargo test` in the
integrator.

## Script commands

Points are window coordinates in logical px (`x,y`) or anchors:
`@chat`, `@sidebar`, `@members`, `@guilds` (list viewports), `@settings` (the
settings scroll area), `@video` (the video player painted last), `@window`. `.tl .tr .bl .br .t .b .l .r` pick a corner
or edge instead of the centre, `#n` picks list item `n` (`#-1` = last), and
`+dx,dy` offsets the point: `@chat#-1.tl+30,20`, `@chat.b+0,-24`.

| command | effect |
| --- | --- |
| `step <label>` | names the checks that follow |
| `wait <ms>` | drives frames (animations, springs, network) |
| `move <pt>`, `hover <pt> [ms]` | mouse move |
| `down <pt> [left\|right\|middle]`, `up <pt> [btn]` | one press or release |
| `click <pt> [btn]`, `dblclick <pt>` | move, then down + up (click count 1, 2) |
| `drag <from> <to> [steps]` | press, moves with the left button held, release |
| `wheel <pt> <notches>` | mouse wheel, positive = up (towards older messages) |
| `lines <pt> <dy>`, `pixels <pt> <dy>` | one raw scroll event |
| `touchpad <pt> <px> [events]` | a touchpad swipe split in pixel events |
| `key <k>...` | keystrokes (`pageup`, `ctrl-k`, `escape`, `shift-tab`) |
| `type <text>` | one keystroke per character |
| `hold shift\|ctrl\|alt\|none` | modifiers for the following mouse events |
| `action pageup\|pagedown\|top\|bottom` | dispatch the action itself (no key) |
| `select <channel id>` | setup only: open a channel without clicking |
| `jump <message id>` | setup only: jump to a message of the open channel |
| `text <text>` | insert text in the composer (one input change, like a paste or an IME commit; `\n` = new line) |
| `clip_image <file>` | put a PNG or JPEG on the offscreen window's clipboard (never the user's), for `key ctrl-v` |
| `region <name> x,y,w,h` | a screen rectangle for `px:<name>` |
| `shot <name>` | capture `<out>-<name>.png` |
| `record <name> <n>@<fps>` | capture `n` frames in real time, one every `1/fps` s while the window keeps animating: `<out>-<name>-0000.png`… and `<out>-<name>.ffconcat` with the measured time of each frame (`scripts/media/gif.py` turns it into a GIF at the real rate); prints the fps reached |
| `print <probe>...`, `mark <probe>...` | print, or remember for `changed` / `up` / `down` |
| `expect <probe> <op> [value]` | one check |
| `until <ms> <probe> <op> [value]` | wait until the check holds, then report it |

Operators: `== != < > <= >=`, `~` (contains), `changed` / `same` (against the
mark), `up` / `down` (greater / smaller than the mark by more than the value,
0.5 by default).

Probes: `chat_y` (px from the top of the message list), `chat_end` (px above the
bottom), `chat_max`,
`chat_item`, `chat_items`, `chat_thumb` (height of the chat scrollbar thumb), `chat_skeleton` (1 while the loading placeholders at the top are on screen), `at_bottom`, `sidebar_y`, `members_y`, `guilds_y`,
`settings`, `settings_y`, `channel`, `guild`, `msgmenu`, `navmenu`,
`usermenu`, `menus`, `popout`, `profile`, `modal`, `switcher`, `composer`,
`ac` (autocomplete rows), `video_t` (current time of the clicked video, -1
when none), `fullscreen`, `px:<region>` (hash of the region's pixels, for
"the video frame changes"), `at:<point>` (prints where a point resolves).

More probes: `flash` (message being highlighted after a jump), `replying`, `messages` (loaded
messages), `reached_start`, `oldest`, `last`, `last_mine`, `last_reply`, `last_files`, `last_reactions` (own reactions), `last_burst` (own super reactions), `last_pinned`, `last_edited`, `last_forward`, `last_flags`, `inbox`, `threads_panel`, `pins` (loaded pins, -1 closed, -2 loading), `members_shown`, `help`, `tail` (last six messages as id:kind), `dupes` (duplicate ids in the open channel), `rows` (messages/rendered rows),
`last_embeds`, `last_thread`, `last_app` (application of the last message), `last_text` (sandbox
channel only), `sending`, `uploads`, `chat_error`, `typing` (typing users in the open channel),
`typing_any`, `typing_label`, `unread_guilds`, `mention_total`, `unreads` (`guild:mentions` of every
unread guild), `search` (loaded results, -1 when closed), `picker` (open picker tab), `audio_t`
(position of the audio or voice message being played, -1 when none), `theme`, `zoom`, `font`,
`notify_on`, `notify_sounds`, `settings_error`.

Writes from a script: with `PHX_WRITE=1`, `real_shots` sends, replies, uploads, creates threads
and polls (text or question starting with `[Test Phoenix]`), schedules messages, votes, and runs
application commands and components, ONLY in the sandbox channel `1576039645127385188` and in the
threads it created there. A GIF picked in the picker is sent there with the test prefix in front of
its link. With `PHX_WRITE=1` the messages this run sent can also be edited, deleted, reacted to (super reactions included), pinned and forwarded to the sandbox itself; voice messages go out only with `PHOENIX_VOICE_MESSAGE_TONE=<secs>` set, so the recorder takes a generated 440 Hz tone and never the microphone. Everything else stays refused. Run setting changes with `XDG_CONFIG_HOME` pointing at a
copy of the config, so the local preferences of the user's Phoenix are never rewritten.

## Frame sequences and GIFs

`shot` advances the clock by 4 s per capture, so it cannot show motion. `record` can: it
captures the window while time runs normally, so hover transitions, menus that open,
springs, smooth scrolling and playing GIFs come out as a sequence. A capture takes time
in a dev build, so the fps reached can be lower than the one asked; the `.ffconcat` file
keeps the real timing either way.

```
move @chat#-1.l+60,0
record hover 45@30
```

The same scripts run on demo data with the `screenshot` example (`PHX_SCRIPT`), which is
what public media (PRs, issues, README) must use:

```sh
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 PHX_SCRIPT=hover.qa int-bin/screenshot out/after.png
python3 scripts/media/gif.py out/after-hover.ffconcat out/after-hover.gif
```

`thread [id]` opens a thread of the selected channel in the sidebar: the given id, or the
newest thread started from a loaded message; without an id and with no thread in the
loaded messages it opens the « new thread » sidebar form instead (same width rules). The
probe `thread` prints the open thread id, `form` for that form, `-` otherwise.
