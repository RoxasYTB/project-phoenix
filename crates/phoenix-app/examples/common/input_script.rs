use std::{collections::HashMap, time::Duration};

use gpui::{
    AnyWindowHandle, Bounds, Entity, HeadlessAppContext, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, PlatformInput, Point, ScrollDelta, ScrollEvent, ScrollSource, TouchPhase, point, px,
};
use phoenix_app::app::PhoenixApp;

const SANDBOX: u64 = 1576039645127385188;

pub const SETTLE_BUDGET: Duration = Duration::from_millis(1500);

thread_local! {
    static SETTLE_CUT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn settle(cx: &HeadlessAppContext) {
    let t0 = std::time::Instant::now();
    while cx.background_executor().tick() {
        if t0.elapsed() >= SETTLE_BUDGET {
            let n = SETTLE_CUT.with(|c| {
                c.set(c.get() + 1);
                c.get()
            });
            if n == 1 || n.is_power_of_two() {
                println!("QA note settle stopped after {} ms with tasks still queued ({n} times so far)", t0.elapsed().as_millis());
            }
            break;
        }
    }
}

pub fn script_lines(script: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in script.lines() {
        let raw = raw.trim_end_matches('\r');
        let lead = raw.trim_start();
        if lead.starts_with("text ") || lead == "text" {
            out.push(lead.to_string());
            continue;
        }
        out.extend(raw.split(';').map(str::trim).map(str::to_string));
    }
    out
}

fn split_item(a: &str) -> anyhow::Result<(&str, Option<i64>, &str)> {
    let Some((name, rest)) = a.split_once('#') else {
        let (name, corner) = a.split_once('.').unwrap_or((a, "c"));
        return Ok((name, None, corner));
    };
    let (item, corner) = rest.split_once('.').unwrap_or((rest, "c"));
    let item = item.parse().map_err(|_| anyhow::anyhow!("bad item #{item}"))?;
    Ok((name, Some(item), corner))
}

fn last(a: &PhoenixApp) -> Option<&phoenix_app::model::Message> {
    a.selected_channel.and_then(|c| a.messages.get(&c)).and_then(|e| e.msgs.last())
}

pub struct Env<'a> {
    pub win: AnyWindowHandle,
    pub app: Entity<PhoenixApp>,
    pub stem: String,
    pub shoot: &'a dyn Fn(&mut HeadlessAppContext, &str) -> anyhow::Result<()>,
}

#[derive(Default)]
pub struct Report {
    pub ok: usize,
    pub ko: Vec<String>,
    pub shots: Vec<String>,
}

struct Run<'a, 'b> {
    env: &'a Env<'b>,
    step: String,
    marks: HashMap<String, String>,
    regions: HashMap<String, [f32; 4]>,
    mods: Modifiers,
    mouse: Point<Pixels>,
    report: Report,
}

pub fn run(cx: &mut HeadlessAppContext, env: &Env, script: &str) -> anyhow::Result<Report> {
    let mut r = Run {
        env,
        step: String::new(),
        marks: HashMap::new(),
        regions: HashMap::new(),
        mods: Modifiers::default(),
        mouse: point(px(1.), px(1.)),
        report: Report::default(),
    };
    let (scale, zoom, vp) = cx.update_window(env.win, |_, window, _| (window.scale_factor(), window.page_zoom(), window.viewport_size()))?;
    println!("QA before scale={scale} zoom={zoom} viewport={vp:?}");
    let scale = std::env::var("PHX_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(scale);
    cx.update_window(env.win, |_, window, _| window.activate_window())?;
    r.frames(cx, 300);
    cx.update_window(env.win, |_, window, _| window.set_scale_factor(scale))?;
    r.frames(cx, 200);
    let (zoom, vp) = cx.update_window(env.win, |_, window, _| (window.page_zoom(), window.viewport_size()))?;
    println!("QA window scale={scale} zoom={zoom} viewport={vp:?}");
    for (n, line) in script_lines(script).iter().map(String::as_str).enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Err(e) = r.exec(cx, line) {
            let msg = phoenix_app::diagnostics::one_line(&format!("[{}] line {n} `{line}`: {e:#}", r.step));
            println!("QA KO {msg}");
            r.report.ko.push(msg);
        }
    }
    println!("QA summary ok={} ko={}", r.report.ok, r.report.ko.len());
    for k in &r.report.ko {
        println!("QA failed {k}");
    }
    for s in &r.report.shots {
        println!("QA shot {s}");
    }
    Ok(r.report)
}

fn button(s: Option<&str>) -> MouseButton {
    match s {
        Some("right") => MouseButton::Right,
        Some("middle") => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

fn num(s: Option<&str>, what: &str) -> anyhow::Result<f32> {
    s.ok_or_else(|| anyhow::anyhow!("missing {what}"))?.parse().map_err(|_| anyhow::anyhow!("bad {what}"))
}

fn corner(b: Bounds<Pixels>, c: &str) -> Point<Pixels> {
    let (l, t, r, bt) = (b.origin.x, b.origin.y, b.origin.x + b.size.width, b.origin.y + b.size.height);
    let (cx, cy) = (b.center().x, b.center().y);
    match c {
        "tl" => point(l, t),
        "tr" => point(r, t),
        "bl" => point(l, bt),
        "br" => point(r, bt),
        "t" => point(cx, t),
        "b" => point(cx, bt),
        "l" => point(l, cy),
        "r" => point(r, cy),
        _ => point(cx, cy),
    }
}

impl Run<'_, '_> {
    fn frames(&mut self, cx: &mut HeadlessAppContext, ms: u64) {
        let t0 = std::time::Instant::now();
        let mut last = t0;
        loop {
            let now = std::time::Instant::now();
            cx.advance_clock(now - last);
            last = now;
            cx.simulate_frame_request(self.env.win);
            settle(cx);
            if t0.elapsed() >= Duration::from_millis(ms) {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn record(&mut self, cx: &mut HeadlessAppContext, name: &str, n: usize, fps: f64) -> anyhow::Result<()> {
        let (sf, vp) = cx.update_window(self.env.win, |_, window, _| (window.scale_factor(), window.viewport_size()))?;
        let (w, h) = ((f32::from(vp.width) * sf).round() as u32, (f32::from(vp.height) * sf).round() as u32);
        let t0 = std::time::Instant::now();
        let mut last = t0;
        let mut frames = Vec::with_capacity(n);
        for i in 0..n {
            let due = t0 + Duration::from_secs_f64(i as f64 / fps);
            loop {
                let now = std::time::Instant::now();
                cx.advance_clock(now - last);
                last = now;
                cx.simulate_frame_request(self.env.win);
                settle(cx);
                let now = std::time::Instant::now();
                if now >= due {
                    break;
                }
                std::thread::sleep((due - now).min(Duration::from_millis(8)));
            }
            let at = t0.elapsed();
            cx.update_window(self.env.win, |_, window, _| window.refresh())?;
            let mut img = cx.capture_screenshot(self.env.win)?;
            let (cw, ch) = (w.min(img.width()), h.min(img.height()));
            if (cw, ch) != (img.width(), img.height()) {
                img = image::imageops::crop_imm(&img, 0, 0, cw, ch).to_image();
            }
            let file = format!("{}-{name}-{i:04}.png", self.env.stem);
            img.save(&file)?;
            frames.push((file, at));
        }
        let mut list = String::from("ffconcat version 1.0\n");
        for (i, (file, at)) in frames.iter().enumerate() {
            let next = frames.get(i + 1).map_or(Duration::from_secs_f64(1. / fps), |(_, t)| t.saturating_sub(*at));
            let base = std::path::Path::new(file).file_name().and_then(|f| f.to_str()).unwrap_or(file);
            list.push_str(&format!("file '{base}'\nduration {:.4}\n", next.as_secs_f64()));
        }
        if let Some((file, _)) = frames.last() {
            let base = std::path::Path::new(file).file_name().and_then(|f| f.to_str()).unwrap_or(file);
            list.push_str(&format!("file '{base}'\n"));
        }
        let concat = format!("{}-{name}.ffconcat", self.env.stem);
        std::fs::write(&concat, list)?;
        let span = frames.last().map_or(0., |(_, t)| t.as_secs_f64());
        let real = if span > 0. { n.saturating_sub(1) as f64 / span } else { fps };
        println!("QA record [{}] {name}: {n} frames, asked {fps} fps, got {real:.1} fps, {concat}", self.step);
        self.report.shots.push(concat);
        Ok(())
    }

    fn send(&mut self, cx: &mut HeadlessAppContext, ev: PlatformInput) -> anyhow::Result<()> {
        cx.update_window(self.env.win, |_, window, cx| {
            window.dispatch_event(ev, cx);
        })?;
        cx.simulate_frame_request(self.env.win);
        settle(cx);
        Ok(())
    }

    fn anchor(&self, cx: &mut HeadlessAppContext, name: &str, item: Option<i64>) -> anyhow::Result<Bounds<Pixels>> {
        let v = self.env.app.clone();
        let win = self.env.win;
        let vp = cx.update_window(win, |_, window, _| window.viewport_size())?;
        cx.update(|cx| {
            let a = v.read(cx);
            let list = match name {
                "chat" => Some(&a.message_list),
                "sidebar" => Some(&a.sidebar_list),
                "members" => Some(&a.member_list),
                "guilds" => Some(&a.guild_list),
                _ => None,
            };
            if let Some(list) = list {
                if let Some(i) = item {
                    let n = list.item_count() as i64;
                    let ix = if i < 0 { n + i } else { i };
                    anyhow::ensure!((0..n).contains(&ix), "{name} has {n} items, not #{i}");
                    return list.bounds_for_item(ix as usize).ok_or_else(|| anyhow::anyhow!("{name} item #{ix} is not laid out"));
                }
                let b = list.viewport_bounds();
                anyhow::ensure!(b.size.height > px(0.), "{name} is not on screen");
                return Ok(b);
            }
            match name {
                "settings" => {
                    let b = a.settings.as_ref().map(|s| s.scroll.bounds()).ok_or_else(|| anyhow::anyhow!("settings are closed"))?;
                    anyhow::ensure!(b.size.height > px(0.), "settings scroll area is not laid out");
                    Ok(b)
                }
                "window" => Ok(Bounds { origin: point(px(0.), px(0.)), size: vp }),
                "video" => phoenix_app::ui::chat::video_player::shot_bounds(cx).ok_or_else(|| anyhow::anyhow!("no video player on screen")),
                _ => anyhow::bail!("unknown anchor @{name}"),
            }
        })
    }

    fn point(&self, cx: &mut HeadlessAppContext, spec: &str) -> anyhow::Result<Point<Pixels>> {
        let (base, off) = match spec.find('+') {
            Some(i) => (&spec[..i], Some(&spec[i + 1..])),
            None => (spec, None),
        };
        let mut p = if let Some(a) = base.strip_prefix('@') {
            let (a, item, c) = split_item(a).map_err(|e| anyhow::anyhow!("{e} in {spec}"))?;
            let p = corner(self.anchor(cx, a, item)?, c);
            let k = cx.update_window(self.env.win, |_, window, _| window.page_zoom())?;
            point(p.x * k, p.y * k)
        } else {
            let (x, y) = base.split_once(',').ok_or_else(|| anyhow::anyhow!("bad point {spec}"))?;
            point(px(x.parse()?), px(y.parse()?))
        };
        if let Some(off) = off {
            let (dx, dy) = off.split_once(',').ok_or_else(|| anyhow::anyhow!("bad offset in {spec}"))?;
            p = point(p.x + px(dx.parse()?), p.y + px(dy.parse()?));
        }
        Ok(p)
    }

    fn mv(&mut self, cx: &mut HeadlessAppContext, at: Point<Pixels>, pressed: Option<MouseButton>) -> anyhow::Result<()> {
        self.mouse = at;
        self.send(cx, PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: pressed, modifiers: self.mods }))
    }

    fn down(&mut self, cx: &mut HeadlessAppContext, at: Point<Pixels>, b: MouseButton, count: usize) -> anyhow::Result<()> {
        self.send(cx, PlatformInput::MouseDown(MouseDownEvent { button: b, position: at, modifiers: self.mods, click_count: count, first_mouse: false }))
    }

    fn up(&mut self, cx: &mut HeadlessAppContext, at: Point<Pixels>, b: MouseButton, count: usize) -> anyhow::Result<()> {
        self.send(cx, PlatformInput::MouseUp(MouseUpEvent { button: b, position: at, modifiers: self.mods, click_count: count }))
    }

    fn scroll(&mut self, cx: &mut HeadlessAppContext, at: Point<Pixels>, delta: ScrollDelta) -> anyhow::Result<()> {
        if at != self.mouse {
            self.mv(cx, at, None)?;
        }
        self.send(
            cx,
            PlatformInput::Scroll(ScrollEvent { position: at, delta, modifiers: self.mods, touch_phase: TouchPhase::Moved, source: ScrollSource::Pointer }),
        )
    }

    fn key(&mut self, cx: &mut HeadlessAppContext, ks: Keystroke) -> anyhow::Result<()> {
        cx.update_window(self.env.win, |_, window, cx| {
            window.dispatch_keystroke(ks, cx);
        })?;
        cx.simulate_frame_request(self.env.win);
        settle(cx);
        Ok(())
    }

    fn region_hash(&mut self, cx: &mut HeadlessAppContext, name: &str) -> anyhow::Result<String> {
        let [x, y, w, h] = *self.regions.get(name).ok_or_else(|| anyhow::anyhow!("unknown region {name}"))?;
        cx.update_window(self.env.win, |_, window, _| window.refresh())?;
        cx.simulate_frame_request(self.env.win);
        settle(cx);
        let img = cx.capture_screenshot(self.env.win)?;
        let s = cx.update_window(self.env.win, |_, window, _| window.scale_factor())?;
        let (x0, y0) = ((x * s) as u32, (y * s) as u32);
        let (x1, y1) = ((((x + w) * s) as u32).min(img.width()), (((y + h) * s) as u32).min(img.height()));
        let mut hash: u64 = 0xcbf29ce484222325;
        for yy in y0..y1 {
            for xx in x0..x1 {
                for c in &img.get_pixel(xx, yy).0[..3] {
                    hash = (hash ^ u64::from(*c >> 2)).wrapping_mul(0x100000001b3);
                }
            }
        }
        Ok(format!("{hash:016x}"))
    }

    fn probe(&mut self, cx: &mut HeadlessAppContext, name: &str) -> anyhow::Result<String> {
        if let Some(r) = name.strip_prefix("px:") {
            return self.region_hash(cx, r);
        }
        if let Some(at) = name.strip_prefix("at:") {
            let p = self.point(cx, at)?;
            return Ok(format!("{:.0},{:.0}", f32::from(p.x), f32::from(p.y)));
        }
        let v = self.env.app.clone();
        cx.update(|cx| {
            let a = v.read(cx);
            let b = |x: bool| if x { "1".to_string() } else { "0".to_string() };
            let y = |p: Point<Pixels>| format!("{:.1}", -f32::from(p.y));
            Ok(match name {
                "chat_y" => y(a.message_list.scroll_px_offset_for_scrollbar()),
                "chat_max" => format!("{:.1}", f32::from(a.message_list.max_offset_for_scrollbar().y)),
                "chat_end" => format!("{:.1}", f32::from(a.message_list.max_offset_for_scrollbar().y) + f32::from(a.message_list.scroll_px_offset_for_scrollbar().y)),
                "chat_item" => a.message_list.logical_scroll_top().item_ix.to_string(),
                "chat_items" => a.message_list.item_count().to_string(),
                "chat_thumb" => format!("{:.1}", phoenix_app::ui::chat::scrollbar::auto_thumb(&a.message_list)),
                "chat_skeleton" => b(phoenix_app::ui::chat::rows::skeleton_shown(a)),
                "at_bottom" => b(a.chat.at_bottom),
                "sidebar_y" => y(a.sidebar_list.scroll_px_offset_for_scrollbar()),
                "members_y" => y(a.member_list.scroll_px_offset_for_scrollbar()),
                "guilds_y" => y(a.guild_list.scroll_px_offset_for_scrollbar()),
                "settings" => b(a.settings.is_some()),
                "settings_y" => a.settings.as_ref().map_or_else(|| "-".into(), |s| y(s.scroll.offset())),
                "channel" => a.selected_channel.map_or_else(|| "-".into(), |c| c.0.to_string()),
                "guild" => a.selected_guild.map_or_else(|| "-".into(), |g| g.0.to_string()),
                "msgmenu" => b(a.chat.menu.is_some()),
                "navmenu" => b(phoenix_app::ui::nav_menus::is_open()),
                "usermenu" => b(phoenix_app::ui::user_menu::is_open()),
                "popout" => b(a.people.popout.is_some()),
                "profile" => b(a.people.modal.is_some()),
                "modal" => b(phoenix_app::actions::with_modal(|m| m.is_some())),
                "switcher" => b(a.switcher.is_some()),
                "menus" => b(a.chat.menu.is_some() || phoenix_app::ui::nav_menus::is_open() || phoenix_app::ui::user_menu::is_open()),
                "composer" => a.composer.read(cx).value().to_string(),
                "ac" => cx
                    .try_global::<phoenix_app::ui::autocomplete::Autocomplete>()
                    .and_then(|s| s.active.as_ref())
                    .map_or(0, |s| s.items.len())
                    .to_string(),
                "video_t" => phoenix_app::ui::chat::video_player::shot_time(cx).map_or_else(|| "-1".into(), |t| format!("{t:.2}")),
                "fullscreen" => b(phoenix_app::ui::chat::video_player::fullscreen_layer(cx).is_some()),
                "flash" => a.chat.flash.map_or_else(|| "-".into(), |m| m.0.to_string()),
                "replying" => a.replying_to.map_or_else(|| "-".into(), |m| m.0.to_string()),
                "messages" => a.selected_channel.and_then(|c| a.messages.get(&c)).map_or(0, |e| e.msgs.len()).to_string(),
                "reached_start" => b(a.selected_channel.and_then(|c| a.messages.get(&c)).is_some_and(|e| e.reached_start)),
                "oldest" => a.selected_channel.and_then(|c| a.messages.get(&c)).and_then(|e| e.msgs.first()).map_or_else(|| "-".into(), |m| m.id.0.to_string()),
                "last" => last(a).map_or_else(|| "-".into(), |m| m.id.0.to_string()),
                "last_mine" => b(last(a).is_some_and(|m| m.author == a.me.id)),
                "last_reply" => last(a).and_then(|m| m.reply_to).map_or_else(|| "-".into(), |m| m.0.to_string()),
                "last_files" => last(a).map_or(0, |m| m.attachments.len()).to_string(),
                "last_embeds" => last(a).map_or(0, |m| m.embeds.len()).to_string(),
                "last_thread" => b(last(a).is_some_and(|m| m.thread.is_some())),
                "last_reactions" => last(a).map_or(0, |m| m.reactions.iter().filter(|r| r.me).count()).to_string(),
                "last_burst" => last(a).map_or(0, |m| m.reactions.iter().filter(|r| r.me_burst).count()).to_string(),
                "last_pinned" => b(last(a).is_some_and(|m| m.pinned)),
                "last_edited" => b(last(a).is_some_and(|m| m.edited)),
                "last_forward" => b(last(a).is_some_and(|m| m.forward.is_some())),
                "last_flags" => last(a).map_or(0, |m| m.flags).to_string(),
                "tail" => a
                    .selected_channel
                    .and_then(|c| a.messages.get(&c))
                    .map_or_else(|| "-".into(), |e| e.msgs.iter().rev().take(6).map(|m| format!("{}:{:?}", m.id.0 % 100_000, m.kind)).collect::<Vec<_>>().join(",")),
                "dupes" => a
                    .selected_channel
                    .and_then(|c| a.messages.get(&c))
                    .map_or(0, |e| {
                        let mut ids: Vec<u64> = e.msgs.iter().map(|m| m.id.0).collect();
                        let n = ids.len();
                        ids.sort_unstable();
                        ids.dedup();
                        n - ids.len()
                    })
                    .to_string(),
                "rows" => a.selected_channel.and_then(|c| a.messages.get(&c)).map_or_else(|| "-".into(), |e| format!("{}/{}", e.msgs.len(), e.rendered.len())),
                "inbox" => b(a.parity.inbox.open),
                "threads_panel" => b(a.parity.threads_open),
                "thread" => a.chat.thread.as_ref().map_or_else(|| if a.thread_form_open() { "form".into() } else { "-".into() }, |t| t.thread.id.0.to_string()),
                "pins" => a.chat.pins.as_ref().map_or_else(|| "-1".into(), |p| if p.loading { "-2".into() } else { p.msgs.len().to_string() }),
                "members_shown" => b(a.show_members),
                "help" => b(phoenix_app::shortcuts::help_open()),
                "last_app" => last(a).and_then(|m| m.application_id).map_or_else(|| "-".into(), |m| m.0.to_string()),
                "last_text" => last(a).filter(|m| m.channel.0 == SANDBOX).map_or_else(|| "-".into(), |m| m.content.chars().take(90).collect()),
                "sending" => a.chat.sending.len().to_string(),
                "uploads" => a.chat.uploads.len().to_string(),
                "chat_error" => a.chat.error.as_ref().map_or_else(|| "-".into(), ToString::to_string),
                "typing" => a.selected_channel.and_then(|c| a.typing.get(&c)).map_or(0, Vec::len).to_string(),
                "typing_any" => a.typing.values().map(Vec::len).sum::<usize>().to_string(),
                "typing_label" => a.typing_label.as_ref().map_or_else(|| "-".into(), ToString::to_string),
                "unread_guilds" => a.guilds.iter().filter(|g| g.unread).count().to_string(),
                "mention_total" => a.guilds.iter().map(|g| g.mentions).sum::<u32>().to_string(),
                "unreads" => a
                    .guilds
                    .iter()
                    .filter(|g| g.unread || g.mentions > 0)
                    .map(|g| format!("{}:{}", g.id.0, g.mentions))
                    .collect::<Vec<_>>()
                    .join(","),
                "search" => cx
                    .try_global::<phoenix_app::ui::search::Search>()
                    .and_then(|s| s.0.as_ref())
                    .map_or(-1, |s| s.page.as_ref().map_or(0, |p| p.messages.len() as i64))
                    .to_string(),
                "picker" => phoenix_app::ui::pickers::open_tab(cx).map_or_else(|| "-".into(), |(t, _)| format!("{t:?}")),
                "audio_t" => phoenix_app::ui::chat::voice_message::playback(cx).map_or_else(|| "-1".into(), |t| format!("{t:.2}")),
                "theme" => format!("{:?}", a.prefs.synced.theme),
                "notify_on" => b(phoenix_app::notifications::enabled()),
                "notify_sounds" => b(a.prefs.local.notification_sounds),
                "zoom" => a.prefs.local.zoom.to_string(),
                "font" => format!("{:.0}", a.prefs.local.font_size),
                "settings_error" => a.settings.as_ref().and_then(|s| s.error.as_ref()).map_or_else(|| "-".into(), ToString::to_string),
                _ => anyhow::bail!("unknown probe {name}"),
            })
        })
    }

    fn check(&mut self, cx: &mut HeadlessAppContext, args: &[&str]) -> anyhow::Result<(bool, String)> {
        let probe = *args.first().ok_or_else(|| anyhow::anyhow!("expect needs a probe"))?;
        let op = *args.get(1).ok_or_else(|| anyhow::anyhow!("expect needs an operator"))?;
        let want = args.get(2..).map(|r| r.join(" ")).unwrap_or_default();
        let got = self.probe(cx, probe)?;
        let mark = self.marks.get(probe).cloned();
        let f = |s: &str| s.parse::<f64>().ok();
        let base = match op {
            "changed" | "same" | "up" | "down" => mark.clone().ok_or_else(|| anyhow::anyhow!("no mark for {probe}"))?,
            _ => want.clone(),
        };
        let thr = f(&want).unwrap_or(0.5);
        let ok = match (op, f(&got), f(&base)) {
            ("changed", ..) => got != base,
            ("same", ..) => got == base,
            ("up", Some(g), Some(w)) => g > w + thr,
            ("down", Some(g), Some(w)) => g < w - thr,
            (">", Some(g), Some(w)) => g > w,
            ("<", Some(g), Some(w)) => g < w,
            (">=", Some(g), Some(w)) => g >= w,
            ("<=", Some(g), Some(w)) => g <= w,
            ("==", Some(g), Some(w)) => (g - w).abs() < 1e-6,
            ("!=", Some(g), Some(w)) => (g - w).abs() >= 1e-6,
            ("==", ..) => got == base,
            ("!=", ..) => got != base,
            ("~", ..) => got.contains(&base),
            _ => anyhow::bail!("cannot compare {probe}={got:?} {op} {base:?}"),
        };
        let detail = format!("{probe} {op} {want} (got {got}{})", mark.map(|m| format!(", mark {m}")).unwrap_or_default());
        Ok((ok, detail))
    }

    fn verdict(&mut self, ok: bool, detail: String) {
        if ok {
            println!("QA OK [{}] {detail}", self.step);
            self.report.ok += 1;
        } else {
            let msg = phoenix_app::diagnostics::one_line(&format!("[{}] {detail}", self.step));
            println!("QA KO {msg}");
            self.report.ko.push(msg);
        }
    }

    fn exec(&mut self, cx: &mut HeadlessAppContext, line: &str) -> anyhow::Result<()> {
        let (cmd, raw) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = raw.trim();
        let args: Vec<&str> = rest.split_whitespace().collect();
        match cmd {
            "step" => {
                self.step = rest.to_string();
                println!("QA step {rest}");
            }
            "wait" => self.frames(cx, num(args.first().copied(), "ms")? as u64),
            "move" => {
                let at = self.point(cx, rest)?;
                self.mv(cx, at, None)?;
            }
            "hover" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                self.mv(cx, at, None)?;
                self.frames(cx, num(args.get(1).copied(), "ms").unwrap_or(300.) as u64);
            }
            "down" | "up" | "click" | "dblclick" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                let b = button(args.get(1).copied());
                if at != self.mouse {
                    self.mv(cx, at, None)?;
                }
                match cmd {
                    "down" => self.down(cx, at, b, 1)?,
                    "up" => self.up(cx, at, b, 1)?,
                    _ => {
                        let n = if cmd == "dblclick" { 2 } else { 1 };
                        for c in 1..=n {
                            self.down(cx, at, b, c)?;
                            self.up(cx, at, b, c)?;
                        }
                    }
                }
                self.frames(cx, 150);
            }
            "drag" => {
                let a = self.point(cx, args.first().copied().unwrap_or_default())?;
                let z = self.point(cx, args.get(1).copied().unwrap_or_default())?;
                let steps = num(args.get(2).copied(), "steps").unwrap_or(10.).max(1.) as usize;
                self.mv(cx, a, None)?;
                self.down(cx, a, MouseButton::Left, 1)?;
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let p = point(a.x + (z.x - a.x) * t, a.y + (z.y - a.y) * t);
                    self.mv(cx, p, Some(MouseButton::Left))?;
                    self.frames(cx, 16);
                }
                self.up(cx, z, MouseButton::Left, 1)?;
                self.frames(cx, 150);
            }
            "wheel" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                let notches = num(args.get(1).copied(), "notches")?;
                let n = notches.abs().round().max(1.) as usize;
                for _ in 0..n {
                    self.scroll(cx, at, ScrollDelta::Lines(point(0., 3. * notches.signum())))?;
                    self.frames(cx, 40);
                }
            }
            "lines" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                let dy = num(args.get(1).copied(), "lines")?;
                self.scroll(cx, at, ScrollDelta::Lines(point(0., dy)))?;
            }
            "pixels" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                let dy = num(args.get(1).copied(), "pixels")?;
                self.scroll(cx, at, ScrollDelta::Pixels(point(px(0.), px(dy))))?;
            }
            "touchpad" => {
                let at = self.point(cx, args.first().copied().unwrap_or_default())?;
                let total = num(args.get(1).copied(), "pixels")?;
                let n = num(args.get(2).copied(), "events").unwrap_or(20.).max(1.) as usize;
                for _ in 0..n {
                    self.scroll(cx, at, ScrollDelta::Pixels(point(px(0.), px(total / n as f32))))?;
                    self.frames(cx, 8);
                }
            }
            "key" | "keys" => {
                for k in &args {
                    let ks = Keystroke::parse_with_os(k, gpui::OperatingSystem::Linux).map_err(|e| anyhow::anyhow!("bad keystroke {k}: {e:?}"))?;
                    self.key(cx, ks)?;
                    self.frames(cx, 60);
                }
            }
            "type" => {
                for ch in rest.chars() {
                    let key = if ch == ' ' { "space".to_string() } else { ch.to_lowercase().to_string() };
                    let mods = Modifiers { shift: ch.is_uppercase(), ..Default::default() };
                    self.key(cx, Keystroke { modifiers: mods, key, key_char: Some(ch.to_string()) })?;
                }
                self.frames(cx, 150);
            }
            "hold" => {
                for m in &args {
                    match *m {
                        "shift" => self.mods.shift = true,
                        "ctrl" => self.mods.control = true,
                        "alt" => self.mods.alt = true,
                        _ => self.mods = Modifiers::default(),
                    }
                }
            }
            "action" => {
                let action: Box<dyn gpui::Action> = match rest {
                    "pageup" => Box::new(phoenix_app::shortcuts::PageUp),
                    "pagedown" => Box::new(phoenix_app::shortcuts::PageDown),
                    "top" => Box::new(phoenix_app::shortcuts::ChatTop),
                    "bottom" => Box::new(phoenix_app::shortcuts::ChatBottom),
                    _ => anyhow::bail!("unknown action {rest}"),
                };
                cx.update_window(self.env.win, |_, window, cx| window.dispatch_action(action, cx))?;
                self.frames(cx, 100);
            }
            "region" => {
                let name = args.first().ok_or_else(|| anyhow::anyhow!("region needs a name"))?.to_string();
                let spec = args.get(1).copied().unwrap_or_default();
                let r: Vec<f32> = if let Some(a) = spec.strip_prefix('@') {
                    let b = self.anchor(cx, a, None)?;
                    let k = cx.update_window(self.env.win, |_, window, _| window.page_zoom())?;
                    [b.origin.x, b.origin.y, b.size.width, b.size.height].map(|v| f32::from(v) * k).to_vec()
                } else {
                    spec.split(',').filter_map(|v| v.parse().ok()).collect()
                };
                anyhow::ensure!(r.len() == 4, "region {name} needs x,y,w,h");
                self.regions.insert(name, [r[0], r[1], r[2], r[3]]);
            }
            "shot" => {
                let path = format!("{}-{}.png", self.env.stem, args.first().copied().unwrap_or("shot"));
                (self.env.shoot)(cx, &path)?;
                self.report.shots.push(path);
            }
            "record" => {
                let name = args.first().copied().unwrap_or("rec");
                let spec = args.get(1).copied().unwrap_or("30@30");
                let (n, fps) = spec.split_once('@').unwrap_or((spec, "30"));
                let n: usize = n.parse().map_err(|_| anyhow::anyhow!("bad frame count {n}"))?;
                let fps: f64 = fps.parse().ok().filter(|f: &f64| *f > 0.).ok_or_else(|| anyhow::anyhow!("bad fps {fps}"))?;
                self.record(cx, name, n, fps)?;
            }
            "print" => {
                for p in &args {
                    let v = self.probe(cx, p)?;
                    println!("QA value [{}] {p}={v}", self.step);
                }
            }
            "mark" => {
                for p in &args {
                    let v = self.probe(cx, p)?;
                    println!("QA mark [{}] {p}={v}", self.step);
                    self.marks.insert(p.to_string(), v);
                }
            }
            "expect" => {
                let (ok, detail) = self.check(cx, &args)?;
                self.verdict(ok, detail);
            }
            "until" => {
                let ms = num(args.first().copied(), "timeout")? as u64;
                let t0 = std::time::Instant::now();
                let (mut ok, mut detail) = self.check(cx, &args[1..])?;
                while !ok && t0.elapsed() < Duration::from_millis(ms) {
                    self.frames(cx, 100);
                    (ok, detail) = self.check(cx, &args[1..])?;
                }
                self.verdict(ok, format!("{detail} after {} ms", t0.elapsed().as_millis()));
            }
            "text" => {
                let text = raw.replace("\\n", "\n");
                let v = self.env.app.clone();
                cx.update_window(self.env.win, |_, window, cx| {
                    v.update(cx, |app, cx| {
                        app.composer.update(cx, |s, cx| {
                            s.focus(window, cx);
                            s.insert(text.clone(), window, cx);
                        });
                        phoenix_app::ui::chat::rich_input::sync(app, cx);
                        cx.notify();
                    })
                })?;
                self.frames(cx, 200);
            }
            "clip_image" => {
                let bytes = std::fs::read(rest)?;
                let format = if rest.ends_with(".jpg") || rest.ends_with(".jpeg") { gpui::ImageFormat::Jpeg } else { gpui::ImageFormat::Png };
                let item = gpui::ClipboardItem { entries: vec![gpui::ClipboardEntry::Image(gpui::Image::from_bytes(format, bytes))] };
                cx.update(|cx| cx.write_to_clipboard(item));
            }
            "jump" => {
                let id = phoenix_app::model::Id(rest.parse()?);
                cx.update_window(self.env.win, |_, _, cx| {
                    self.env.app.update(cx, |app, cx| {
                        if let Some(ch) = app.selected_channel {
                            app.jump_to_message(ch, id, cx);
                        }
                    })
                })?;
                self.frames(cx, 1500);
            }
            "thread" => {
                let wanted = if rest.is_empty() { None } else { Some(phoenix_app::model::Id(rest.parse()?)) };
                let found = cx.update_window(self.env.win, |_, window, cx| {
                    self.env.app.update(cx, |app, cx| {
                        let ch = app.selected_channel?;
                        let known = app.messages.get(&ch)?.msgs.iter().rev().filter_map(|m| m.thread.clone()).find(|t| wanted.is_none_or(|w| w == t.id));
                        if let Some(t) = known {
                            let id = t.id;
                            app.open_thread(t, window, cx);
                            return Some(format!("thread {}", id.0));
                        }
                        if wanted.is_some() {
                            return None;
                        }
                        app.open_create_thread(ch, window, cx);
                        app.thread_form_open().then(|| "new thread form".to_string())
                    })
                })?;
                let what = found.ok_or_else(|| anyhow::anyhow!("no thread {rest} in the loaded messages"))?;
                println!("QA value [{}] opened {what}", self.step);
                self.frames(cx, 1500);
            }
            "select" => {
                let id = phoenix_app::model::Id(rest.parse()?);
                cx.update_window(self.env.win, |_, _, cx| {
                    self.env.app.update(cx, |app, cx| {
                        let g = app.channels.get(&id).and_then(|c| c.guild);
                        app.select_guild(g, cx);
                        app.open_channel_chat(id, cx);
                    })
                })?;
                self.frames(cx, 300);
            }
            _ => anyhow::bail!("unknown command {cmd}"),
        }
        Ok(())
    }
}
