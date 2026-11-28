use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{
    Bounds, Context, DispatchPhase, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Styled,
    canvas, div, prelude::*,
};

use crate::app::PhoenixApp;
use crate::theme::{z, zoom};

pub const MIN_W: f32 = 450.;
pub const HANDLE_W: f32 = 8.;

static SAVED: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static DRAG: Cell<bool> = const { Cell::new(false) };
    static RIGHT: Cell<f32> = const { Cell::new(0.) };
    static MAX: Cell<f32> = const { Cell::new(f32::MAX) };
}

pub fn clamp(w: f32, max: f32) -> f32 {
    let w = w.round();
    if w < MIN_W || max < MIN_W {
        MIN_W
    } else if w > max {
        max.round()
    } else {
        w
    }
}

fn path() -> Option<std::path::PathBuf> {
    crate::paths::config_file("thread_sidebar_width")
}

pub fn saved() -> f32 {
    let v = SAVED.load(Ordering::Relaxed);
    if v != 0 {
        return f32::from_bits(v);
    }
    let w = path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(MIN_W).max(MIN_W);
    SAVED.store(w.to_bits(), Ordering::Relaxed);
    w
}

fn set(w: f32, persist: bool) {
    let w = clamp(w, MAX.with(Cell::get));
    SAVED.store(w.to_bits(), Ordering::Relaxed);
    if persist && let Some(p) = path() {
        let _ = p.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(p, format!("{w}\n"));
    }
}

pub fn handle(width: Pixels, max: Pixels, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    MAX.with(|m| m.set(f32::from(max) / zoom()));
    let weak = cx.entity().downgrade();
    let probe = canvas(
        move |b: Bounds<Pixels>, _, _| RIGHT.with(|r| r.set(f32::from(b.origin.x + b.size.width + width))),
        move |_, _, window, _| {
            if !DRAG.with(Cell::get) {
                return;
            }
            let w1 = weak.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !DRAG.with(Cell::get) {
                    return;
                }
                set((RIGHT.with(Cell::get) - f32::from(ev.position.x)) / zoom(), false);
                let _ = w1.update(cx, |_, cx| cx.notify());
            });
            let w2 = weak.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !DRAG.with(Cell::get) {
                    return;
                }
                DRAG.with(|d| d.set(false));
                set(saved(), true);
                let _ = w2.update(cx, |_, cx| cx.notify());
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div().w(z(HANDLE_W)).flex_none().h_full().relative().child(probe).child(
        div()
            .id("thread-sidebar-resize")
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(z(HANDLE_W))
            .cursor_ew_resize()
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                DRAG.with(|d| d.set(true));
                cx.stop_propagation();
            }),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn clamps_between_the_minimum_and_the_room_left() {
        assert_eq!(super::clamp(100., 900.), 450.);
        assert_eq!(super::clamp(1200., 900.), 900.);
        assert_eq!(super::clamp(612.4, 900.), 612.);
        assert_eq!(super::clamp(700., 400.), 450.);
    }
}
