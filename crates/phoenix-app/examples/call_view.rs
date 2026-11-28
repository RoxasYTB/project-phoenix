use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let login_only = false;
    let rest: Vec<String> = args.collect();
    let with_chat = rest.iter().any(|a| a == "chat");
    let focus_grid = rest.iter().any(|a| a == "grid");
    let no_strip = rest.iter().any(|a| a == "nostrip");
    let dm = rest.iter().any(|a| a == "dm");
    let toasts = rest.iter().any(|a| a == "toasts");
    let voice_only = rest.iter().any(|a| a == "voiceonly");
    let pip_drag = rest.iter().find_map(|a| a.strip_prefix("pipdrag=").map(str::to_owned));
    let split = rest.iter().find_map(|a| a.strip_prefix("split=").and_then(|v| v.parse::<f32>().ok()));
    let feedback = rest.iter().find_map(|a| a.strip_prefix("feedback=").map(str::to_owned));
    let menu = rest.iter().find_map(|a| match a.as_str() {
        "menu=more" => Some(phoenix_app::ui::call_menus::TrayMenu::More),
        "menu=streams" => Some(phoenix_app::ui::call_menus::TrayMenu::Streams),
        _ => None,
    });
    let caret = rest.iter().find_map(|a| a.strip_prefix("caret=").map(str::to_owned));
    let vc_status = rest.iter().find_map(|a| a.strip_prefix("status=").map(str::to_owned));
    let count = rest.iter().find_map(|a| a.strip_prefix("n=").and_then(|v| v.parse::<u64>().ok()));
    let win_size = rest
        .iter()
        .find_map(|a| {
            let (w, h) = a.strip_prefix("win=")?.split_once('x')?;
            Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
        })
        .unwrap_or((1280., 800.));
    let sub = rest.iter().find_map(|a| a.strip_prefix("sub=").and_then(|v| v.parse::<usize>().ok()));
    let theme = rest.iter().find_map(|a| {
        let (p, s) = a.strip_prefix("theme=")?.split_once(',')?;
        Some((u32::from_str_radix(p, 16).ok()?, u32::from_str_radix(s, 16).ok()?))
    });

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), || {
        gpui_platform::current_headless_renderer()
    });
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Italic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]
    .iter()
    .filter_map(|p| std::fs::read(p).ok().map(Into::into))
    .collect();
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
        phoenix_app::demo_media::install(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(win_size.0), px(win_size.1)), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    let pump = |cx: &mut HeadlessAppContext, n: usize| {
        for _ in 0..n {
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    pump(&mut cx, 2);
    if !login_only {
        let v = view.borrow().clone().expect("view");
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                app.login_with_token = true;
                app.login_token.update(cx, |s, cx| s.set_value("mock-token", window, cx));
                app.submit_login(window, cx);
            });
        })?;
        pump(&mut cx, 30);
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                use phoenix_app::model::{Id, VoiceFlags};
                let me = app.me.id;
                let target = if dm {
                    let mut dms: Vec<_> = app.channels.values().filter(|c| c.guild.is_none()).map(|c| c.id).collect();
                    dms.sort_by_key(|c| c.0);
                    dms.first().copied()
                } else {
                    app.channels.values().find(|c| c.name.as_ref() == "Lounge").map(|c| c.id)
                };
                let Some(lounge) = target else { return };
                if dm && let Some(ch) = app.channels.get_mut(&lounge) {
                    ch.voice_users = vec![me, Id(3), Id(4), Id(5), Id(6), Id(7)];
                }
                if let Some(ch) = app.channels.get_mut(&lounge)
                    && !ch.voice_users.contains(&me)
                {
                    ch.voice_users.insert(0, me);
                }
                if !dm && let Some(ch) = app.channels.get_mut(&lounge) {
                    for u in [Id(3), Id(4), Id(5), Id(6), Id(7)] {
                        if !ch.voice_users.contains(&u) {
                            ch.voice_users.push(u);
                        }
                    }
                }
                if let Some(n) = count
                    && let Some(ch) = app.channels.get_mut(&lounge)
                {
                    ch.voice_users = std::iter::once(me).chain((3..).map(Id).take(n.saturating_sub(1) as usize)).collect();
                }
                phoenix_app::voice::start_time::set_channel_status(lounge.0, vc_status.as_deref());
                app.voice_channel = Some(lounge);
                app.voice.apply(phoenix_app::voice::VoiceUiEvent::Status(phoenix_app::voice::VoiceStatus::Connected));
                app.open_channel_chat(lounge, cx);
                phoenix_app::ui::call_view::show(lounge);
                phoenix_app::ui::call_view::set_chat_open(lounge, with_chat);
                if focus_grid {
                    phoenix_app::ui::call_view::set_focus(phoenix_app::ui::call_view::Focus::Grid);
                }
                phoenix_app::ui::call_view::touch();
                phoenix_app::ui::call_view::set_strip_open(!no_strip);
                phoenix_app::ui::voice_panel::set_flags(me, VoiceFlags { stream: !voice_only, ..Default::default() });
                phoenix_app::ui::call_view::set_demo_still(me, (!voice_only).then(|| phoenix_app::demo_media::stream_still(1280, 720)));
                phoenix_app::ui::call_view::set_split_height(split, false);
                phoenix_app::ui::voice_panel::set_flags(Id(3), VoiceFlags { video: !voice_only, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(4), VoiceFlags { self_mute: true, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(5), VoiceFlags { server_mute: true, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(6), VoiceFlags { server_deaf: true, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(7), VoiceFlags { self_deaf: true, ..Default::default() });
                app.voice.speaking.insert(Id(3));
                if let Some(th) = theme {
                    for u in [me, Id(4)] {
                        app.people.profiles.insert(u, phoenix_app::presence::Profile { user: u, theme: Some(th), ..Default::default() });
                    }
                }
                phoenix_app::ui::call_menus::open(menu);
                match caret.as_deref() {
                    Some("mic") => phoenix_app::ui::voice_panel::toggle_call_audio_menu(gpui::point(gpui::px(560.), gpui::px(700.))),
                    Some("camera") => phoenix_app::ui::voice_panel::toggle_camera_menu(gpui::point(gpui::px(620.), gpui::px(700.))),
                    _ => {}
                }
                if toasts {
                    let lines = ["anyone up for a match tonight?", "", "the stream looks great from here"];
                    for (i, text) in lines.into_iter().enumerate() {
                        let attachments = if text.is_empty() {
                            vec![phoenix_app::model::Attachment { filename: "screenshot.png".into(), kind: phoenix_app::model::AttachmentKind::Image, ..Default::default() }]
                        } else {
                            Vec::new()
                        };
                        let m = phoenix_app::model::Message {
                            id: Id(990_000 + i as u64),
                            channel: lounge,
                            author: Id(3 + i as u64),
                            content: text.into(),
                            attachments,
                            timestamp: chrono::Utc::now().timestamp(),
                            ..Default::default()
                        };
                        app.handle_event(phoenix_app::backend::AppEvent::MessageCreate(m), cx);
                    }
                }
                if let Some(f) = feedback.as_deref() {
                    use phoenix_app::ui::modals::stream_feedback::State;
                    let st = match f {
                        "list" => State { open: true, ..Default::default() },
                        "picked" => State { problem: Some(2), ..Default::default() },
                        "done" => State { submitted: true, ..Default::default() },
                        _ => State::default(),
                    };
                    phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::StreamFeedback(st), None);
                }
                if pip_drag.is_some() {
                    app.selected_channel = app.channels.values().find(|c| c.guild.is_some() && c.name.as_ref() == "general").map(|c| c.id);
                }
                if let Some(ix) = sub {
                    phoenix_app::ui::context_menu::debug_open_submenu(ix);
                }
            });
        })?;
        pump(&mut cx, 10);
    }
    let point = |name: &str| {
        std::env::var(name).ok().and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some(gpui::point(px(x.trim().parse::<f32>().ok()?), px(y.trim().parse::<f32>().ok()?)))
        })
    };
    let move_to = |cx: &mut HeadlessAppContext, at: gpui::Point<gpui::Pixels>| -> anyhow::Result<()> {
        let ev = gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent { position: at, pressed_button: None, modifiers: gpui::Modifiers::default() });
        cx.update_window(win.into(), |_, window, cx| {
            window.dispatch_event(ev, cx);
        })?;
        Ok(())
    };
    let snap = |cx: &mut HeadlessAppContext, path: &str| -> anyhow::Result<()> {
        for _ in 0..2 {
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            pump(cx, 2);
        }
        let img = cx.capture_screenshot(win.into())?;
        img.save(path)?;
        println!("wrote {path}");
        Ok(())
    };
    if let Some(dir) = pip_drag.as_deref() {
        use phoenix_app::ui::pip::{self, Corner};
        std::fs::create_dir_all(dir)?;
        let send = |cx: &mut HeadlessAppContext, ev: gpui::PlatformInput| -> anyhow::Result<()> {
            cx.update_window(win.into(), |_, window, cx| {
                window.dispatch_event(ev, cx);
            })?;
            Ok(())
        };
        let mut clock = 1_000_i64;
        let mut frame = 0_usize;
        pip::freeze_clock(Some(clock));
        let mut shot = |cx: &mut HeadlessAppContext| -> anyhow::Result<()> {
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            cx.run_until_parked();
            let img = cx.capture_screenshot(win.into())?;
            img.save(format!("{dir}/f{frame:04}.png"))?;
            frame += 1;
            Ok(())
        };
        let (vw, vh) = win_size;
        let centre = |c: Corner| {
            let (x, y) = pip::corner_position(c, vw, vh, (pip::PIP_W, pip::PIP_H));
            (x + pip::PIP_W / 2., y + pip::PIP_H / 2.)
        };
        pip::set_corner(Corner::BottomRight);
        pip::set_width(pip::PIP_W);
        shot(&mut cx)?;
        let scenarios = [
            (Corner::BottomRight, (300., 250.), 500),
            (Corner::TopLeft, (1000., 220.), 500),
            (Corner::TopRight, (280., 600.), 500),
            (Corner::BottomLeft, (1000., 600.), 500),
            (Corner::BottomRight, (720., 430.), 110),
        ];
        let at = |x: f32, y: f32| gpui::point(px(x), px(y));
        for (from, to, dur) in scenarios {
            let start = centre(from);
            send(&mut cx, gpui::PlatformInput::MouseDown(gpui::MouseDownEvent { button: gpui::MouseButton::Left, position: at(start.0, start.1), modifiers: gpui::Modifiers::default(), click_count: 1, first_mouse: false }))?;
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            cx.run_until_parked();
            let steps = (dur as f32 / 16.).round() as usize;
            let mut last = start;
            for i in 1..=steps {
                clock += 16;
                pip::freeze_clock(Some(clock));
                let u = i as f32 / steps as f32;
                last = (start.0 + (to.0 - start.0) * u, start.1 + (to.1 - start.1) * u);
                send(&mut cx, gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent { position: at(last.0, last.1), pressed_button: Some(gpui::MouseButton::Left), modifiers: gpui::Modifiers::default() }))?;
                shot(&mut cx)?;
            }
            send(&mut cx, gpui::PlatformInput::MouseUp(gpui::MouseUpEvent { button: gpui::MouseButton::Left, position: at(last.0, last.1), modifiers: gpui::Modifiers::default(), click_count: 1 }))?;
            for _ in 0..80 {
                clock += 16;
                pip::freeze_clock(Some(clock));
                shot(&mut cx)?;
            }
        }
        pip::freeze_clock(None);
        println!("wrote {frame} frames to {dir}");
        return Ok(());
    }
    if let Some(at) = point("PHX_MOUSE") {
        move_to(&mut cx, at)?;
        pump(&mut cx, 4);
        if let Ok(spec) = std::env::var("PHX_HOVER_ANIM") {
            for ms in spec.split(',').filter_map(|v| v.trim().parse::<u64>().ok()) {
                phoenix_app::ui::hover_icon::freeze(Some(std::time::Duration::from_millis(ms)));
                snap(&mut cx, &format!("{}-t{ms:04}.png", out.trim_end_matches(".png")))?;
            }
            phoenix_app::ui::hover_icon::freeze(None);
        }
        if let Some(away) = point("PHX_MOUSE_LEAVE") {
            move_to(&mut cx, away)?;
            pump(&mut cx, 4);
            snap(&mut cx, &format!("{}-leave.png", out.trim_end_matches(".png")))?;
        }
    }
    if let Ok(seq) = std::env::var("PHX_SPEAK_SEQ") {
        let who = phoenix_app::model::Id(std::env::var("PHX_SPEAK_USER").ok().and_then(|v| v.parse().ok()).unwrap_or(3));
        let v = view.borrow().clone().expect("view");
        if let Ok(url) = std::env::var("PHX_SPEAK_AVATAR") {
            cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, _| {
                    if let Some(u) = app.users.get_mut(&who) {
                        u.avatar = phoenix_app::model::ImageRef::Url(url.into());
                    }
                });
            })?;
            pump(&mut cx, 40);
        }
        for (i, on) in seq.chars().filter(|c| *c == '0' || *c == '1').enumerate() {
            cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, cx| {
                    if on == '1' {
                        app.voice.speaking.insert(who);
                    } else {
                        app.voice.speaking.remove(&who);
                    }
                    cx.notify();
                });
            })?;
            std::thread::sleep(Duration::from_millis(100));
            cx.advance_clock(Duration::from_millis(100));
            snap(&mut cx, &format!("{}-s{i:03}.png", out.trim_end_matches(".png")))?;
        }
    }
    phoenix_app::ui::call_view::touch();
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(std::time::Duration::from_secs(2));
    let _ = cx.capture_screenshot(win.into())?;
    phoenix_app::ui::call_view::touch();
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(std::time::Duration::from_secs(2));
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    Ok(())
}
