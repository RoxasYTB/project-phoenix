use crate::theme::z;
use std::rc::Rc;

use gpui::{
    AnyElement, App, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, Styled,
    Window, div, prelude::*,
};


use crate::theme::{c, generated::idx, tok};

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct Confirm {
    pub title: SharedString,
    pub body: Vec<AnyElement>,
    pub confirm_label: SharedString,
    pub danger: bool,
    pub on_confirm: Action,
    pub on_cancel: Action,
}

pub fn frame(id: &'static str, on_cancel: Action, card: impl IntoElement) -> AnyElement {
    frame_named(id, None, on_cancel, card)
}

pub fn frame_named(id: &'static str, label: Option<SharedString>, on_cancel: Action, card: impl IntoElement) -> AnyElement {
    crate::media::lru::deferred_full(
        div()
            .id(id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, w, cx| on_cancel(w, cx))
            .child(crate::ui::popout_motion::modal_backdrop(id, div().absolute().top_0().left_0().size_full().bg(c(tok(idx::BACKGROUND_SCRIM)))))
            .child(crate::ui::popout_motion::modal_card(
                id,
                crate::ui::focus_ring::trap_modal(
                    crate::ui::a11y::role(div().id("modal-card").debug_selector(|| "modal-card".into()), gpui::Role::Dialog)
                        .when_some(label.filter(|_| crate::ui::a11y::on()), |d, l| d.aria_label(l))
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(card),
            )),
    )
    .with_priority(20)
    .into_any_element()
}

pub fn confirm(m: Confirm) -> AnyElement {
    let close = m.on_cancel.clone();
    confirm_labeled(m, crate::t!("common.cancel"), close)
}

pub fn confirm_labeled(m: Confirm, cancel_label: SharedString, on_close: Action) -> AnyElement {
    use crate::ui::controls::{BtnKind, button};
    let cancel = on_close.clone();
    let cancel2 = m.on_cancel.clone();
    let ok = m.on_confirm.clone();
    let mut body = m.body.into_iter();
    let subtitle = body.next();
    let title = m.title.clone();
    let rest: Vec<AnyElement> = body.collect();
    let has_body = !rest.is_empty();
    let card = div()
        .relative()
        .w(z(if has_body { 480. } else { 400. }))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(
                    div()
                        .pr(z(36.))
                        .text_size(z(20.))
                        .line_height(z(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(m.title),
                )
                .children(subtitle.map(|s| {
                    div()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(c(tok(idx::TEXT_SUBTLE)))
                        .child(s)
                })),
        )
        .when(has_body, |d| {
            d.child(
                div()
                    .pt(z(16.))
                    .pb(z(8.))
                    .pl(z(24.))
                    .pr(z(24.))
                    .flex()
                    .flex_col()
                    .gap(z(20.))
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .children(rest),
            )
        })
        .child(
            crate::ui::focus_ring::ring(
                div()
                    .id("modal-close")
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .size(z(40.))
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                    .on_click(move |_, w, cx| on_close(w, cx))
                    .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
                crate::ui::states::key("modal-close"),
                8.,
            ),
        )
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .gap(z(8.))
                .child(
                    crate::ui::focus_ring::ring(
                        button("modal-cancel", cancel_label, BtnKind::Secondary)
                            .relative()
                            .flex_1()
                            .on_click(move |_, w, cx| cancel2(w, cx)),
                        crate::ui::states::key("modal-cancel"),
                        8.,
                    ),
                )
                .child(
                    crate::ui::focus_ring::ring(
                        button("modal-confirm", m.confirm_label, if m.danger { BtnKind::Critical } else { BtnKind::Primary })
                            .relative()
                            .flex_1()
                            .on_click(move |_, w, cx| ok(w, cx)),
                        crate::ui::states::key("modal-confirm"),
                        8.,
                    ),
                ),
        );
    frame_named("modal-backdrop", Some(title), cancel, card)
}
