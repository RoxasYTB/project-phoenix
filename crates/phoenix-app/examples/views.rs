use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend, ui::settings::AppTheme};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let theme = match args.next().as_deref() {
        Some("light") => AppTheme::Light,
        Some("ash") => AppTheme::Ash,
        Some("onyx") => AppTheme::Onyx,
        _ => AppTheme::Dark,
    };
    let scene = args.next().unwrap_or_else(|| "appearance".into());
    let login_only = scene.starts_with("login");

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
    let h: f32 = std::env::var("PHOENIX_SHOT_H").ok().and_then(|v| v.parse().ok()).unwrap_or(800.);
    let w: f32 = std::env::var("PHOENIX_SHOT_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280.);
    let win = cx.open_window(size(px(w), px(h)), move |window, cx| {
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
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                use phoenix_app::ui::{pickers, settings};
                app.prefs.synced.theme = theme;
                let page = |app: &mut PhoenixApp, p, cx: &mut gpui::Context<PhoenixApp>| {
                    settings::open(app, cx);
                    if let Some(s) = app.settings.as_mut() {
                        s.page = p;
                    }
                };
                match scene.as_str() {
                    "account" => page(app, settings::Page::MyAccount, cx),
                    "status" => app.toggle_status_picker(window, cx),
                    "switcher" => app.open_switcher(window, cx),
                    "emoji" => pickers::open(app, pickers::Tab::Emoji, pickers::EmojiTarget::Composer, window, cx),
                    "reaction" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        let channel = app.selected_channel.unwrap_or_default();
                        pickers::open(app, pickers::Tab::Emoji, pickers::EmojiTarget::Reaction { channel, message: phoenix_app::model::Id(1) }, window, cx);
                    }
                    "gif" => pickers::open(app, pickers::Tab::Gif, pickers::EmojiTarget::Composer, window, cx),
                    "sticker" => pickers::open(app, pickers::Tab::Sticker, pickers::EmojiTarget::Composer, window, cx),
                    "friends" => app.select_guild(None, cx),
                    "friends-all" | "friends-pending" => {
                        app.select_guild(None, cx);
                        app.nav.friends_tab = if scene == "friends-all" {
                            phoenix_app::ui::home::data::FriendsTab::All
                        } else {
                            phoenix_app::ui::home::data::FriendsTab::Pending
                        };
                    }
                    "inbox" | "pins" | "threads" | "search" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        match scene.as_str() {
                            "inbox" => app.toggle_inbox(cx),
                            "pins" => app.toggle_pins(cx),
                            "threads" => app.toggle_threads_popout(cx),
                            _ => phoenix_app::ui::search::open(app, window, cx),
                        }
                    }
                    "pin-confirm" | "unpin-confirm" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        if scene == "unpin-confirm" {
                            app.toggle_pins(cx);
                        }
                    }
                    "members" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                    }
                    "dm-profile" => {
                        app.select_guild(None, cx);
                        let dm = app.channels.values().filter(|c| c.guild.is_none() && c.voice_users.len() == 1).map(|c| c.id).min_by_key(|i| i.0);
                        if let Some(id) = dm {
                            app.select_channel(id, cx);
                        }
                        app.show_members = true;
                    }
                    "dm-menu" | "dm-submenu" => {
                        app.select_guild(None, cx);
                        let dm = app.channels.values().filter(|c| c.guild.is_none() && !c.voice_users.is_empty()).map(|c| c.id).min_by_key(|i| i.0);
                        if let Some(id) = dm {
                            phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Channel(id), gpui::point(px(232.), px(88.)), window);
                            if scene == "dm-submenu" {
                                phoenix_app::ui::context_menu::debug_open_submenu(3);
                            }
                        }
                    }
                    "guild-menu" => {
                        if let Some(g) = app.guilds.first().map(|g| g.id) {
                            phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Guild(g), gpui::point(px(40.), px(120.)), window);
                        }
                    }
                    "channel-menu" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        if let Some(id) = app.selected_channel {
                            phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Channel(id), gpui::point(px(200.), px(120.)), window);
                        }
                    }
                    "member-menu" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        let uid = g.and_then(|g| app.members.get(&g)).and_then(|m| m.iter().map(|m| m.user).find(|u| *u != app.me.id));
                        if let Some(uid) = uid {
                            phoenix_app::ui::user_menu::open(uid, g, gpui::point(px(900.), px(120.)), window);
                        }
                    }
                    "games" => page(app, settings::Page::RegisteredGames, cx),
                    "developer" => page(app, settings::Page::Advanced, cx),
                    "profile" | "profile-modal" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        let me = app.me.id;
                        let uid = if std::env::var_os("PHX_PROFILE_SELF").is_some() {
                            Some(me)
                        } else {
                            g.and_then(|g| app.members.get(&g)).and_then(|m| m.iter().map(|m| m.user).find(|u| *u != me))
                        };
                        if let Some(uid) = uid {
                            if let Ok(text) = std::env::var("PHX_PROFILE_STATUS") {
                                if uid == me {
                                    app.people.custom_status = Some(text.into());
                                } else {
                                    app.people.activities.insert(
                                        uid,
                                        vec![phoenix_app::presence::Activity {
                                            kind: phoenix_app::presence::ActivityKind::Custom,
                                            name: "Custom Status".into(),
                                            details: None,
                                            state: Some(text.into()),
                                            emoji: None,
                                            url: None,
                                            display_type: None,
                                            image: None,
                                            start: None,
                                            application_id: None,
                                            party: None,
                                            platform: None,
                                            flags: 0,
                                        }],
                                    );
                                }
                            }
                            app.open_profile(uid, g, gpui::point(px(1000.), px(200.)), window, cx);
                            let roles = g.and_then(|g| app.roles.get(&g)).map(|r| r.iter().take(3).map(|r| r.id).collect()).unwrap_or_default();
                            app.people.profiles.insert(
                                uid,
                                phoenix_app::presence::Profile {
                                    user: uid,
                                    banner: None,
                                    accent: Some(0x5865F2),
                                    decoration: None,
                                    pronouns: "they/them".into(),
                                    bio: std::env::var("PHX_BIO").unwrap_or_else(|_| "Building native clients in Rust. Coffee first.".into()).into(),
                                    roles,
                                    nick: None,
                                    created: 1_500_000_000,
                                    joined: Some(1_650_000_000),
                                    note: "".into(),
                                    mutual_guilds: 3,
                                    mutual_friends_count: 2,
                                    mutual_friends: app.users.values().filter(|u| u.id != uid && u.id != app.me.id).take(2).cloned().collect(),
                                    mutual_guild_ids: app.guilds.iter().take(3).map(|g| g.id).collect(),
                                    badges: Vec::new(),
                                    theme: std::env::var("PHX_PROFILE_THEME").ok().and_then(|v| {
                                        let (a, b) = v.split_once(',')?;
                                        Some((u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
                                    }),
                                    ..Default::default()
                                },
                            );
                            if scene == "profile-modal" {
                                app.open_profile_modal(uid, cx);
                            }
                        }
                    }
                    "boost" => page(app, settings::Page::ServerBoost, cx),
                    "guild-notify" => {
                        let g = app.guilds[0].id;
                        let st = phoenix_app::ui::modals::guild_notify::State::new(app, g);
                        phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::GuildNotify(st), None);
                    }
                    "incoming-call" => {
                        app.select_guild(None, cx);
                        app.parity.incoming_call = app.dms.first().copied();
                    }
                    "join" => {
                        let id = app.channels.values().find(|c| c.name.as_ref() == "showcase").map(|c| c.id);
                        if let Some(id) = id {
                            app.select_channel(id, cx);
                        }
                    }
                    "events" | "browse" | "guide" => {
                        let g = app.guilds.first().map(|g| g.id);
                        app.select_guild(g, cx);
                        if let Some(g) = g {
                            match scene.as_str() {
                                "events" => phoenix_app::ui::events::open(app, g, cx),
                                "browse" => phoenix_app::ui::community::open_tab(app, g, true, cx),
                                _ => phoenix_app::ui::guide::open(app, g, cx),
                            }
                        }
                    }
                    "forum" | "post" => {
                        let id = app.channels.values().find(|c| c.kind == phoenix_app::model::ChannelKind::Forum).map(|c| c.id);
                        if let Some(id) = id {
                            app.select_channel(id, cx);
                        }
                    }
                    _ => page(app, settings::Page::Appearance, cx),
                }
                cx.notify();
            });
        })?;
        pump(&mut cx, 4);
        if scene == "pin-confirm" || scene == "unpin-confirm" {
            let unpin = scene == "unpin-confirm";
            for _ in 0..40 {
                let ready = cx.update_window(win.into(), |_, _, cx| {
                    let app = v.read(cx);
                    app.selected_channel.and_then(|ch| app.messages.get(&ch)).is_some_and(|x| x.msgs.iter().any(|m| !m.content.is_empty()))
                })?;
                if ready {
                    break;
                }
                pump(&mut cx, 2);
            }
            for _ in 0..10 {
                cx.update_window(win.into(), |_, _, cx| {
                    v.update(cx, |app, cx| {
                        if app.chat.confirm_pin.is_some() {
                            return;
                        }
                        if let Some(ch) = app.selected_channel {
                            let last = app.messages.get(&ch).and_then(|x| x.msgs.iter().rev().find(|m| !m.content.is_empty()).cloned());
                            if let Some(mut m) = last {
                                m.pinned = unpin;
                                app.request_pin_message(ch, m, false, cx);
                            }
                        }
                    });
                })?;
                pump(&mut cx, 6);
                let state = cx.update_window(win.into(), |_, _, cx| {
                    let app = v.read(cx);
                    (app.selected_channel, app.chat.confirm_pin.as_ref().map(|(c, m)| (*c, m.id)))
                })?;
                eprintln!("views: pin scene channel={:?} confirm={:?}", state.0, state.1);
            }
        }
        if scene == "post" {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| {
                    let post = app
                        .selected_channel
                        .and_then(|f| app.parity.forums.get(&f))
                        .and_then(|f| f.posts.first())
                        .map(|p| phoenix_app::model::ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count });
                    if let Some(r) = post {
                        app.open_thread(r, window, cx);
                    }
                });
            })?;
            pump(&mut cx, 4);
        }
    }
    if login_only {
        let v = view.borrow().clone().expect("view");
        let who = match scene.as_str() {
            "login-mfa-sms" => "sms@phoenix.demo",
            "login-mfa-key" => "key@phoenix.demo",
            "login-mfa" | "login-mfa-error" => "totp@phoenix.demo",
            "login-invalid" => "invalid@phoenix.demo",
            _ => "mfa@phoenix.demo",
        };
        if !scene.starts_with("login-qr") && scene != "login" {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| {
                    app.login_email.update(cx, |s, cx| s.set_value(who, window, cx));
                    app.login_password.update(cx, |s, cx| s.set_value("demo", window, cx));
                    app.submit_login(window, cx);
                });
            })?;
            pump(&mut cx, 10);
        }
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                use phoenix_app::auth::{self, MfaSlide};
                match scene.as_str() {
                    "login-mfa-select" => auth::set_slide(app, MfaSlide::Select, cx),
                    "login-mfa-backup" => auth::set_slide(app, MfaSlide::Backup, cx),
                    "login-mfa-backup-code" => auth::set_slide(app, MfaSlide::BackupAck, cx),
                    "login-qr" => {
                        if let Some((modules, size)) = auth::qr_matrix("https://discord.com/ra/PhoenixDemoFingerprintPhoenixDemoFingerprin") {
                            app.login_ui.qr = auth::QrState::Code { url: String::new(), modules, size };
                        }
                    }
                    "login-qr-scanned" => {
                        app.login_ui.qr = auth::QrState::Scanned { name: "phoenix".into(), avatar: auth::scanned_avatar("80351110224678912", None) };
                    }
                    "login-qr-loading" => app.login_ui.qr = auth::QrState::Connecting,
                    "login-mfa-error" => {
                        app.login_mfa.update(cx, |s, cx| s.set_value("000000", window, cx));
                        auth::submit_mfa(app, cx);
                    }
                    _ => {}
                }
                cx.notify();
            });
        })?;
        pump(&mut cx, 10);
    }
    if let Some((x, y)) = std::env::var("PHX_MOUSE").ok().and_then(|v| {
        let (x, y) = v.split_once(',')?;
        Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
    }) {
        for _ in 0..2 {
            let ev = gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent { position: gpui::point(px(x), px(y)), pressed_button: None, modifiers: gpui::Modifiers::default() });
            cx.update_window(win.into(), |_, window, cx| {
                window.dispatch_event(ev, cx);
            })?;
            pump(&mut cx, 4);
        }
    }
    let frames = if std::env::var_os("PHX_MOCK_DECO").is_some() || std::env::var_os("PHX_WAIT_IMAGES").is_some() { 20 } else { 1 };
    for _ in 0..frames {
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(&mut cx, 2);
    }
    cx.advance_clock(std::time::Duration::from_secs(2));
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(std::time::Duration::from_secs(2));
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    if let Ok(spec) = std::env::var("PHX_INBOX_SCROLL") {
        let v = view.borrow().clone().expect("view");
        let stem = out.trim_end_matches(".png").to_string();
        for y in phoenix_app::ui::inbox::inbox_scroll_frames(&spec) {
            cx.update(|cx| {
                phoenix_app::ui::inbox::set_inbox_scroll(y);
                v.update(cx, |_, cx| cx.notify());
            });
            for _ in 0..2 {
                cx.update_window(win.into(), |_, window, _| window.refresh())?;
                pump(&mut cx, 2);
            }
            let img = cx.capture_screenshot(win.into())?;
            img.save(format!("{stem}-s{:04}.png", y as u32))?;
        }
    }
    Ok(())
}
