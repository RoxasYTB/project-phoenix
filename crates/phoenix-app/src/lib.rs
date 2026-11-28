pub mod actions;
pub mod app;
pub mod assets;
pub mod auth;
pub mod autostart;
pub mod backend;
pub mod commands;
pub mod deeplink;
pub mod diagnostics;
pub mod discord_backend;
pub mod grouping;
pub mod guild_live;
pub mod history_ui;
pub mod history_detached;
pub mod hwaccel;
pub mod keybinds;
pub mod clips;
pub mod i18n;
pub mod fonts;
pub mod icons;
pub mod idle;
pub mod logging;
pub mod markdown;
pub mod member_index;
pub mod media;
pub mod memory;
pub mod demo_media;
pub mod mock;
pub mod mock_showcase;
pub mod mock_stage;
pub mod model;
pub mod notifications;
pub mod paths;
pub mod perf;
pub mod presence;
pub mod richtext;
pub mod saved_accounts;
pub mod search_highlight;
pub mod shortcuts;
pub mod sounds;
pub mod spell;
pub mod splash;
pub mod streamer;
pub mod theme;
pub mod thread_recent;
pub mod tts;
#[cfg(target_os = "linux")]
pub mod tray;
#[cfg(not(target_os = "linux"))]
pub mod tray {
    pub fn running() -> bool {
        false
    }
    pub fn spawn(_: &mut gpui::App) {}
    pub fn set_target(_: &mut gpui::App, _: gpui::WeakEntity<crate::app::PhoenixApp>) {}
}
pub mod ui;
pub mod updater;
pub mod voice;

use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, Pixels, Size, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, point, px,
    size,
};
use gpui_component::{Root, theme::{Theme, ThemeMode}};

pub const APP_ID: &str = "discord-phoenix";

fn window_icon() -> Option<Arc<image::RgbaImage>> {
    image::load_from_memory_with_format(assets::WINDOW_ICON_PNG, image::ImageFormat::Png)
        .ok()
        .map(|i| Arc::new(i.to_rgba8()))
}

const RESTORED_SIZE: Size<Pixels> = Size { width: px(1280.), height: px(720.) };
const MIN_SIZE: Size<Pixels> = Size { width: px(940.), height: px(500.) };

fn main_window_bounds(display: Option<Bounds<Pixels>>) -> WindowBounds {
    let Some(area) = display else {
        return WindowBounds::Maximized(Bounds::new(point(px(0.), px(0.)), RESTORED_SIZE));
    };
    let restored = size(RESTORED_SIZE.width.min(area.size.width), RESTORED_SIZE.height.min(area.size.height));
    let origin = point(
        area.origin.x + (area.size.width - restored.width) * 0.5,
        area.origin.y + (area.size.height - restored.height) * 0.5,
    );
    WindowBounds::Maximized(Bounds::new(origin, restored))
}

pub fn window_options(cx: &App) -> WindowOptions {
    let display = cx.primary_display();
    WindowOptions {
        window_bounds: Some(main_window_bounds(display.as_ref().map(|d| d.visible_bounds()))),
        display_id: display.map(|d| d.id()),
        titlebar: Some(TitlebarOptions {
            title: Some("Discord Phoenix".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(APP_ID.into()),
        icon: window_icon(),
        window_min_size: Some(MIN_SIZE),
        ..Default::default()
    }
}

pub fn init(cx: &mut App) {
    init_base(cx);
    fonts::register(cx);
    perf::mark("init_done");
}

pub fn init_deferred(cx: &mut App) {
    init_base(cx);
    fonts::register_deferred(cx);
    perf::mark("init_done");
}

fn init_base(cx: &mut App) {
    memory::init();
    gpui_component::init(cx);
    gpui::set_smooth_wheel_scroll(std::env::var_os("PHX_INSTANT_WHEEL").is_none());
    Theme::change(ThemeMode::Dark, None, cx);
    app::bind_keys(cx);
    media::lru::init(cx);
}

pub fn open_main_window(backend: Arc<dyn backend::Backend>, cx: &mut App) -> Option<gpui::AnyWindowHandle> {
    tray::spawn(cx);
    notifications::spawn(cx);
    let opts = window_options(cx);
    cx.open_window(opts, move |window, cx| {
        window.set_window_title("Discord Phoenix");
        let view = cx.new(|cx| app::PhoenixApp::new(backend, window, cx));
        tray::set_target(cx, view.downgrade());
        deeplink::set_target(view.downgrade(), cx);
        streamer::watch(view.downgrade(), cx);
        tts::enable_output();
        let weak = view.downgrade();
        window.on_window_should_close(cx, move |w, cx| {
            if let Some(view) = weak.upgrade() {
                view.update(cx, |app, _| app.persist_chat_position());
            }
            let hide = tray::running() && weak.upgrade().is_some_and(|v| v.read(cx).prefs.local.minimize_to_tray);
            if hide {
                w.minimize_window();
            }
            !hide
        });
        view.read(cx).focus.clone().focus(window, cx);
        cx.new(|cx| Root::new(view, window, cx))
    })
    .ok()
    .map(Into::into)
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn main_window_opens_maximized_centered_on_primary() {
        let area = Bounds::new(point(px(1920.), px(0.)), size(px(2560.), px(1400.)));
        let WindowBounds::Maximized(b) = main_window_bounds(Some(area)) else { panic!("not maximized") };
        assert_eq!(b.size, RESTORED_SIZE);
        assert_eq!(b.origin, point(px(1920. + 640.), px(340.)));
    }

    #[test]
    fn restored_size_fits_small_displays() {
        let area = Bounds::new(point(px(0.), px(0.)), size(px(1024.), px(600.)));
        let WindowBounds::Maximized(b) = main_window_bounds(Some(area)) else { panic!("not maximized") };
        assert_eq!(b.size, size(px(1024.), px(600.)));
        assert_eq!(b.origin, point(px(0.), px(0.)));
    }

    #[test]
    fn maximized_without_display() {
        assert!(matches!(main_window_bounds(None), WindowBounds::Maximized(_)));
    }
}
