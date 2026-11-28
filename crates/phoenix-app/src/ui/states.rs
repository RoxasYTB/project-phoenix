use crate::theme::{c, generated::idx, tok, z};
use crate::ui::Icon;
use gpui::{Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, svg};
use gpui::prelude::FluentBuilder as _;

pub fn composer_button(id: &'static str, i: Icon) -> gpui::Stateful<Div> {
    composer_button_ext(id, i, 20., true)
}

pub fn composer_lottie(id: &str) -> Option<crate::icons::lottie_icons::Lottie> {
    use crate::icons::lottie_icons::Lottie;
    match id {
        "gift" => Some(Lottie::Gift),
        "gif" => Some(Lottie::Gif),
        "sticker" => Some(Lottie::Sticker),
        _ => None,
    }
}

pub fn composer_button_ext(id: &'static str, i: Icon, glyph: f32, hover_bg: bool) -> gpui::Stateful<Div> {
    composer_button_full(id, i, glyph, hover_bg, false)
}

pub fn composer_button_full(id: &'static str, i: Icon, glyph: f32, hover_bg: bool, active: bool) -> gpui::Stateful<Div> {
    use gpui::IntoElement as _;
    let k = key(("composer-btn", id));
    let fallback = move || {
        svg()
            .path(i.path())
            .size(z(glyph))
            .flex_none()
            .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
            .group_hover(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
            .id("glyph")
            .group_active(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
            .into_any_element()
    };
    let size = glyph * crate::theme::zoom();
    let colors = (c(tok(idx::INTERACTIVE_TEXT_DEFAULT)), c(tok(idx::INTERACTIVE_TEXT_HOVER)));
    let glyph_el = match composer_lottie(id) {
        Some(kind) => crate::ui::hover_icon::icon(k, kind, false, size, colors, fallback),
        None if id == "emoji" => crate::ui::hover_icon::emoji_button(k, active, size, colors.0, fallback),
        None => fallback(),
    };
    let el = track(div().id(id), k)
        .relative()
        .when(hover_bg, |d| d.child(fade_layer(k, gpui::transparent_black(), c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED)), 200, 8.)))
        .group(id)
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .cursor_pointer()
        .child(glyph_el)
        .when(composer_lottie(id).is_some(), |d| d.on_click(move |_, _, _| crate::ui::hover_icon::clicked(k)));
    let dx = if id == "emoji" { -4. } else { 0. };
    crate::ui::focus_ring::ring_inset(el, k, if hover_bg { 8. } else { 4. }, dx, 4.)
}

thread_local! {
    static HOVER: std::cell::RefCell<std::collections::HashMap<u64, (bool, std::time::Instant)>> = Default::default();
}

pub fn key(id: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    h.finish()
}

pub fn state(key: u64) -> Option<(bool, std::time::Instant)> {
    HOVER.with(|h| h.borrow().get(&key).copied())
}

pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 + Copy + 'static {
    move |t: f32| {
        let t = t.clamp(0., 1.);
        let bez = |p1: f32, p2: f32, s: f32| 3. * (1. - s) * (1. - s) * s * p1 + 3. * (1. - s) * s * s * p2 + s * s * s;
        let dx = |s: f32| 3. * (1. - s) * (1. - s) * x1 + 6. * (1. - s) * s * (x2 - x1) + 3. * s * s * (1. - x2);
        let mut s = t;
        for _ in 0..8 {
            let d = dx(s);
            if d.abs() < 1e-6 {
                break;
            }
            s = (s - (bez(x1, x2, s) - t) / d).clamp(0., 1.);
        }
        let (mut lo, mut hi) = (0f32, 1f32);
        if (bez(x1, x2, s) - t).abs() > 1e-4 {
            for _ in 0..30 {
                s = (lo + hi) / 2.;
                if bez(x1, x2, s) < t { lo = s } else { hi = s }
            }
        }
        bez(y1, y2, s)
    }
}

pub fn hovered(key: u64) -> bool {
    HOVER.with(|h| h.borrow().get(&key).is_some_and(|s| s.0))
}

pub fn set_hover(key: u64, over: bool) -> bool {
    HOVER.with(|h| {
        let mut h = h.borrow_mut();
        let prev = h.get(&key).is_some_and(|s| s.0);
        h.insert(key, (over, std::time::Instant::now()));
        prev != over
    })
}

pub fn track(el: gpui::Stateful<Div>, key: u64) -> gpui::Stateful<Div> {
    el.child(hover_guard(key)).on_hover(move |over, window, _| {
        if set_hover(key, *over) {
            window.refresh();
        }
    })
}

thread_local! {
    static FRAME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static PAINTED: std::cell::RefCell<std::collections::HashMap<u64, u64>> = Default::default();
}

fn mark_painted(key: u64) {
    let frame = FRAME.with(|f| f.get());
    PAINTED.with(|p| p.borrow_mut().insert(key, frame));
}

pub fn begin_frame() {
    let frame = FRAME.with(|f| {
        let n = f.get() + 1;
        f.set(n);
        n
    });
    let gone = sweep(frame);
    for key in gone {
        forget(key);
    }
}

fn sweep(frame: u64) -> Vec<u64> {
    PAINTED.with(|p| {
        let mut p = p.borrow_mut();
        let gone: Vec<u64> = p.iter().filter(|(_, seen)| **seen + 1 < frame).map(|(k, _)| *k).collect();
        for k in &gone {
            p.remove(k);
        }
        gone.into_iter().filter(|k| hovered(*k)).collect()
    })
}

pub fn blur_all() {
    let keys: Vec<u64> = HOVER.with(|h| h.borrow().iter().filter(|(_, s)| s.0).map(|(k, _)| *k).collect());
    for key in keys {
        forget(key);
    }
}

pub fn roll_place(line: f32, pos: f32) -> ((f32, f32), (f32, f32)) {
    let d = 1.07 * line;
    ((-d * pos, 1. - pos), (d * (1. - pos), pos))
}

pub fn hover_roll(key: u64, line: f32, default: gpui::AnyElement, hovered: gpui::AnyElement) -> Div {
    use gpui::{IntoElement, PresentTransitionExt};
    const MS: u64 = 220;
    let (over, at, animating) = match state(key) {
        Some((o, a)) => (o, a, a.elapsed() < std::time::Duration::from_millis(MS)),
        None => (false, std::time::Instant::now(), false),
    };
    let wrap = |el: gpui::AnyElement, front: bool| {
        let base = div().w_full().relative().child(el);
        if front { base } else { base.absolute().left_0().top_0().size_full() }
    };
    let layer = |el: gpui::AnyElement, front: bool| -> gpui::AnyElement {
        let pick = move |pos: f32| if front { roll_place(line, pos).0 } else { roll_place(line, pos).1 };
        let (to_pos, from_pos) = if over { (1., 0.) } else { (0., 1.) };
        let ((y1, o1), (y0, o0)) = (pick(to_pos), pick(from_pos));
        let el = wrap(el, front).top(gpui::px(y1)).opacity(if animating { 1. } else { o1 });
        let t = animating.then(|| {
            gpui::Transition::ease(std::time::Duration::from_millis(MS), gpui::CubicBezier(0.25, 0.1, 0.25, 1.))
                .start(at)
                .translate(gpui::point(gpui::px(0.), gpui::px(y0 - y1)), gpui::point(gpui::px(0.), gpui::px(0.)))
                .opacity(o0, o1)
                .keep_masks()
        });
        el.present_transition(gpui::ElementId::NamedInteger(if front { "roll-front".into() } else { "roll-back".into() }, key), t).into_any_element()
    };
    div().relative().w_full().h(gpui::px(line)).overflow_hidden().child(layer(default, true)).child(layer(hovered, false))
}

pub fn mix(a: gpui::Hsla, b: gpui::Hsla, t: f32) -> gpui::Hsla {
    let (a, b): (gpui::Rgba, gpui::Rgba) = (a.into(), b.into());
    let l = |x: f32, y: f32| x + (y - x) * t;
    gpui::Rgba { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }.into()
}

pub fn fade_layer(key: u64, rest: gpui::Hsla, hover: gpui::Hsla, ms: u64, radius: f32) -> gpui::AnyElement {
    fade_layer_pressed(key, rest, hover, None, ms, radius)
}

pub fn fade_layer_pressed(
    key: u64,
    rest: gpui::Hsla,
    hover: gpui::Hsla,
    pressed: Option<(&'static str, gpui::Hsla)>,
    ms: u64,
    radius: f32,
) -> gpui::AnyElement {
    use gpui::{IntoElement, PresentTransitionExt};
    let state = HOVER.with(|h| h.borrow().get(&key).copied());
    let on = state.is_some_and(|s| s.0);
    let (from, to) = if on { (rest, hover) } else { (hover, rest) };
    let mut layer = div().absolute().top_0().left_0().size_full().rounded(z(radius)).id("fade-layer").bg(to);
    if let Some((group, col)) = pressed {
        layer = layer.group_active(group, move |s| s.bg(col));
    }
    let t = state.and_then(|(_, at)| {
        (at.elapsed().as_millis() < ms as u128)
            .then(|| gpui::Transition::ease(std::time::Duration::from_millis(ms), EASE_OUT_QUAD).start(at).background(from, to))
    });
    layer.present_transition(gpui::ElementId::NamedInteger("fade-pt".into(), key), t).into_any_element()
}

pub const EASE_OUT_QUAD: gpui::CubicBezier = gpui::CubicBezier(0.25, 0.46, 0.45, 0.94);

pub const EASE: gpui::CubicBezier = gpui::CubicBezier(0.25, 0.1, 0.25, 1.);

pub fn opacity_fade(key: u64, ms: u64, el: Div) -> gpui::AnyElement {
    use gpui::{IntoElement, PresentTransitionExt};
    let state = state(key);
    let on = state.is_some_and(|s| s.0);
    let t = state.and_then(|(_, at)| {
        let (from, to) = if on { (0., 1.) } else { (1., 0.) };
        (at.elapsed().as_millis() < ms as u128)
            .then(|| gpui::Transition::ease(crate::ui::settings::effects::motion(ms), EASE).start(at).opacity(from, to))
    });
    let shown = on || t.is_some();
    el.opacity(if shown { 1. } else { 0. }).id("opacity-fade").present_transition(gpui::ElementId::NamedInteger("opacity-pt".into(), key), t).into_any_element()
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}

pub fn below_tip(text: gpui::SharedString, button_w: f32, button_h: f32) -> gpui::AnyElement {
    use gpui::{AnchoredPositionMode, FontWeight, IntoElement, anchored, deferred, point};
    let bg = c(tok(idx::BACKGROUND_SURFACE_HIGH));
    let mut caret = div().flex().flex_col().items_center();
    for i in 0..5 {
        caret = caret.child(div().w(z(1. + 2. * i as f32)).h(z(1.)).bg(bg));
    }
    const SPAN: f32 = 240.;
    let card = div()
        .max_w(z(200.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .bg(bg)
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .whitespace_nowrap()
        .child(text);
    let tip = deferred(
        anchored().position_mode(AnchoredPositionMode::Local).position(point(z(button_w / 2. - SPAN / 2.), z(button_h + 6.))).child(tip_zoom(
            "below-tip",
            (0.5, 0.),
            div().w(z(SPAN)).flex().flex_col().items_center().child(caret).child(card),
        )),
    )
    .with_priority(2);
    div().absolute().top_0().left_0().child(tip).into_any_element()
}

pub fn header_button(id: &'static str, path: gpui::SharedString, selected: bool, label: gpui::SharedString) -> gpui::Stateful<Div> {
    let k = key(("hdr-btn", id));
    let rest = c(tok(if selected { idx::ICON_STRONG } else { idx::ICON_MUTED }));
    let hover = c(tok(if selected { idx::ICON_STRONG } else { idx::ICON_SUBTLE }));
    track(div().id(id), k)
        .group(id)
        .relative()
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            svg()
                .path(path)
                .size(z(20.))
                .flex_none()
                .text_color(rest)
                .group_hover(id, move |s| s.text_color(hover)),
        )
        .children(hovered(k).then(|| below_tip(label, 32., 32.)))
        .map(|d| crate::ui::focus_ring::ring(d, k, 4.))
}

pub fn header_button_disabled(id: &'static str, path: gpui::SharedString, label: gpui::SharedString) -> gpui::Stateful<Div> {
    let k = key(("hdr-btn", id));
    track(div().id(id), k)
        .relative()
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(svg().path(path).size(z(20.)).flex_none().text_color(c(tok(idx::CHANNEL_ICON))))
        .children(hovered(k).then(|| below_tip(label, 32., 32.)))
}

pub fn fade_to(key: u64, on: bool, rest: gpui::Hsla, hover: gpui::Hsla, ms: u64, radius: f32) -> gpui::AnyElement {
    HOVER.with(|h| {
        let mut h = h.borrow_mut();
        match h.get(&key) {
            Some(s) if s.0 == on => {}
            None => {
                h.insert(key, (on, std::time::Instant::now() - std::time::Duration::from_secs(1)));
            }
            Some(_) => {
                h.insert(key, (on, std::time::Instant::now()));
            }
        }
    });
    fade_layer(key, rest, hover, ms, radius)
}

pub fn tip_transition(origin: (f32, f32)) -> gpui::Transition {
    gpui::Transition::ease(crate::ui::settings::effects::motion(100), EASE_OUT_QUAD).origin(origin.0, origin.1).scale(0.95, 1.)
}

pub fn tip_zoom(id: &'static str, origin: (f32, f32), el: impl gpui::IntoElement + 'static) -> gpui::AnyElement {
    use gpui::{IntoElement, PresentTransitionExt};
    el.present_transition(id, Some(tip_transition(origin))).into_any_element()
}

pub fn forget(key: u64) {
    HOVER.with(|h| {
        let mut h = h.borrow_mut();
        if h.get(&key).is_none_or(|s| s.0) {
            h.insert(key, (false, std::time::Instant::now() - std::time::Duration::from_secs(1)));
        }
    });
}

pub fn hover_guard(key: u64) -> gpui::AnyElement {
    use gpui::IntoElement;
    gpui::canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            mark_painted(key);
            if hovered(key) && !hitbox.is_hovered(window) {
                forget(key);
                window.refresh();
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    #[test]
    fn hover_of_an_anchor_that_stopped_painting_is_dropped() {
        let (kept, gone) = (key("sweep-kept"), key("sweep-gone"));
        set_hover(kept, true);
        set_hover(gone, true);
        FRAME.with(|f| f.set(10));
        mark_painted(kept);
        mark_painted(gone);
        begin_frame();
        mark_painted(kept);
        begin_frame();
        assert!(hovered(kept));
        assert!(!hovered(gone));
    }

    #[test]
    fn window_blur_clears_every_hover() {
        let k = key("blur-all");
        set_hover(k, true);
        blur_all();
        assert!(!hovered(k));
    }
}
