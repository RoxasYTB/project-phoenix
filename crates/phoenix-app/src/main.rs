#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use phoenix_app::{assets::Assets, backend::Backend, discord_backend::DiscordBackend, mock::MockBackend};

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallopt(param: i32, value: i32) -> i32;
}

fn limit_malloc_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
        // SAFETY: called first in main, before any other thread exists; mallopt only adjusts glibc allocator tuning.
        unsafe {
            mallopt(-8, 2);
        }
    }
}

fn main() {
    limit_malloc_arenas();
    phoenix_app::logging::init();
    phoenix_app::hwaccel::apply_at_startup();
    phoenix_app::actions::guard::set_writes_enabled(phoenix_app::ui::settings::LocalPrefs::load().protected_writes());
    phoenix_app::perf::start();
    phoenix_app::autostart::refresh();
    if phoenix_api::rest::guard::agent_session() {
        phoenix_app::discord_backend::NO_ACKS.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let mock = std::env::var("PHOENIX_MOCK").is_ok_and(|v| !v.is_empty() && v != "0");
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == phoenix_app::deeplink::REGISTER_FLAG) {
        if let Err(e) = phoenix_app::deeplink::register_protocol() {
            eprintln!("could not register discord:// links: {e}");
            std::process::exit(1);
        }
        return;
    }
    let link = phoenix_app::deeplink::Message { token: phoenix_app::deeplink::launch_token(), url: phoenix_app::deeplink::url_from_args(&args) };
    let single = !mock && !phoenix_app::deeplink::multi_instance(&args);
    if single && phoenix_app::deeplink::ipc::forward(&link) {
        return;
    }
    let (links, incoming) = phoenix_app::deeplink::channel();
    if single {
        phoenix_app::deeplink::ipc::listen(links.clone());
    }
    if link.url.is_some() {
        let _ = links.unbounded_send(phoenix_app::deeplink::Message { token: None, url: link.url });
    }
    phoenix_app::splash::preload();
    let app = gpui_platform::application().with_assets(Assets);
    app.on_open_urls(move |urls| {
        for url in urls {
            let _ = links.unbounded_send(phoenix_app::deeplink::Message { token: None, url: Some(url) });
        }
    });
    app.run(move |cx| {
        phoenix_app::init_deferred(cx);
        phoenix_app::deeplink::serve(incoming, cx);
        phoenix_app::ui::boot::enable();
        phoenix_app::sounds::init();
        let splash = phoenix_app::splash::open(cx);
        phoenix_app::perf::mark("splash_opened");
        let backend: Arc<dyn Backend> = if mock {
            cx.set_http_client(phoenix_app::demo_media::client());
            Arc::new(MockBackend::new())
        } else {
            let b = DiscordBackend::new();
            cx.set_http_client(Arc::new(phoenix_app::media::cache::CachingHttpClient::new(b.http_client())));
            Arc::new(b)
        };
        cx.spawn(async move |cx| {
            let started = std::time::Instant::now();
            while !phoenix_app::fonts::fallback::ready() && started.elapsed() < SPLASH_MAX {
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            }
            cx.update(|cx| {
                if let Some(s) = splash {
                    phoenix_app::splash::set_status(s, phoenix_app::splash::Status::Starting, cx);
                }
                let main = phoenix_app::open_main_window(backend, cx);
                phoenix_app::perf::mark("main_window_opened");
                bench_hooks(main, cx);
                if let Some(s) = splash {
                    phoenix_app::splash::close(s, cx);
                }
                cx.activate(true);
            });
        })
        .detach();
    });
}

const SPLASH_MAX: std::time::Duration = std::time::Duration::from_secs(3);

fn bench_hooks(main: Option<gpui::AnyWindowHandle>, cx: &mut gpui::App) {
    if (std::env::var_os("PHOENIX_BENCH").is_some() || phoenix_app::perf::enabled())
        && let Some(win) = main
    {
        let _ = win.update(cx, |_, window, _| {
            window.on_next_frame(|_, _| {
                phoenix_app::perf::mark("first_frame");
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                eprintln!("PHOENIX_BENCH first_frame_epoch_ms={ms}");
            });
        });
    }
    if let Some(ms) = std::env::var("PHOENIX_BENCH_EXIT_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(ms)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}
