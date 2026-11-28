use gpui::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div,
    img, prelude::*, px,
};
use gpui_component::input::{Input, InputState};

use crate::{
    app::PhoenixApp,
    auth::QrState,
    backend::BackendError,
    theme::{Palette, c, generated::idx, tok},
};

fn label(text: SharedString, error: Option<&str>) -> impl IntoElement {
    div()
        .mb(px(8.))
        .flex()
        .gap(px(4.))
        .text_size(px(12.))
        .font_weight(FontWeight::BOLD)
        .text_color(c(if error.is_some() { tok(idx::TEXT_FEEDBACK_CRITICAL) } else { Palette::INTERACTIVE }))
        .child(text)
        .map(|d| match error {
            Some(e) => d.child(
                div()
                    .font_weight(FontWeight::NORMAL)
                    .italic()
                    .child(SharedString::from(format!("- {e}"))),
            ),
            None => d.child(div().text_color(c(Palette::RED)).child("*")),
        })
}

fn field(state: &Entity<InputState>, error: bool) -> impl IntoElement {
    div()
        .h(px(40.))
        .px(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(c(tok(if error { idx::BORDER_FEEDBACK_CRITICAL } else { idx::INPUT_BORDER_DEFAULT })))
        .bg(c(tok(if error { idx::INPUT_BACKGROUND_ERROR_DEFAULT } else { idx::INPUT_BACKGROUND_DEFAULT })))
        .flex()
        .items_center()
        .text_size(px(16.))
        .text_color(c(Palette::TEXT_NORMAL))
        .child(Input::new(state).appearance(false).bordered(false))
}

fn backdrop() -> gpui::Div {
    div().size_full().flex().items_center().justify_center().bg(gpui::linear_gradient(
        135.,
        gpui::linear_color_stop(c(0x404EED), 0.),
        gpui::linear_color_stop(c(0x23272A), 1.),
    ))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if app.login_mfa_step && !app.login_with_token {
        return backdrop().child(super::login_mfa::render(app, window, cx)).into_any_element();
    }
    let err = app.login_error.as_ref().map(|e| e.as_ref());
    let (err_login, err_password) = match err.and_then(BackendError::field_errors) {
        Some(fields) => fields,
        None => (err, err),
    };
    let form = div()
        .flex_1()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .flex()
                .justify_center()
                .child(crate::t!("login.welcome")),
        )
        .child(
            div()
                .mt(px(8.))
                .text_size(px(16.))
                .text_color(c(Palette::INTERACTIVE))
                .flex()
                .justify_center()
                .child(if app.login_with_token {
                    crate::t!("login.paste_token")
                } else {
                    crate::t!("login.excited")
                }),
        )
        .child(div().h(px(20.)))
        .map(|d| {
            if app.login_with_token {
                d.child(label(crate::t!("login.token"), err)).child(field(&app.login_token, err.is_some())).child(
                    div()
                        .mt(px(8.))
                        .text_size(px(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("login.token_note")),
                )
            } else {
                d.child(label(crate::t!("login.email"), err_login))
                    .child(field(&app.login_email, err_login.is_some()))
                    .child(div().h(px(20.)))
                    .child(label(crate::t!("login.password"), err_password))
                    .child(field(&app.login_password, err_password.is_some()))
                    .child(
                        div()
                            .mt(px(4.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(Palette::TEXT_LINK))
                            .cursor_pointer()
                            .child(crate::t!("login.forgot")),
                    )
            }
        })
        .child(
            div()
                .id("login-btn")
                .mt(px(20.))
                .h(px(44.))
                .rounded(px(8.))
                .bg(c(Palette::BLURPLE))
                .hover(|s| s.bg(c(0x4752C4)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(gpui::white())
                .on_click(cx.listener(|a, _, w, cx| a.submit_login(w, cx)))
                .child(if app.logging_in { "…".into() } else { crate::t!("login.login") }),
        )
        .child(
            div()
                .mt(px(8.))
                .flex()
                .gap(px(4.))
                .text_size(px(14.))
                .child(div().text_color(c(Palette::TEXT_MUTED)).child(crate::t!("login.need_account")))
                .child(div().text_color(c(Palette::TEXT_LINK)).cursor_pointer().child(crate::t!("login.register"))),
        )
        .child(
            div()
                .id("toggle-token")
                .mt(px(4.))
                .text_size(px(14.))
                .text_color(c(Palette::TEXT_LINK))
                .cursor_pointer()
                .on_click(cx.listener(|a, _, _, cx| {
                    a.login_with_token = !a.login_with_token;
                    a.login_mfa_step = false;
                    a.login_error = None;
                    cx.notify();
                }))
                .child(if app.login_with_token { crate::t!("login.with_email") } else { crate::t!("login.with_token") }),
        );

    let (qr_box, qr_title, qr_text): (AnyElement, SharedString, SharedString) = match &app.login_ui.qr {
        QrState::Code { modules, size, .. } => (
            qr_code(modules, *size),
            crate::t!("login.qr_title"),
            crate::t!("login.qr_hint"),
        ),
        QrState::Scanned(name) => (
            div().text_size(px(48.)).child("✓").into_any_element(),
            crate::t!("login.check_phone"),
            crate::t!("login.qr_scanned", name = name),
        ),
        QrState::Unavailable(e) => (
            div()
                .id("qr-retry")
                .cursor_pointer()
                .text_color(c(Palette::BLURPLE))
                .text_size(px(14.))
                .on_click(cx.listener(|a, _, _, cx| crate::auth::retry_qr(a, cx)))
                .child(crate::t!("common.retry"))
                .into_any_element(),
            crate::t!("login.qr_unavailable"),
            e.clone().into(),
        ),
        _ => (
            div().text_color(gpui::black()).text_size(px(14.)).child(crate::t!("login.generating")).into_any_element(),
            crate::t!("login.qr_title"),
            crate::t!("login.qr_hint"),
        ),
    };
    let qr = div()
        .w(px(240.))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .size(px(176.))
                .rounded(px(4.))
                .bg(gpui::white())
                .p(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .child(qr_box),
        )
        .child(
            div()
                .mt(px(32.))
                .text_size(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(qr_title),
        )
        .child(
            div()
                .mt(px(8.))
                .text_size(px(16.))
                .text_color(c(Palette::INTERACTIVE))
                .text_center()
                .child(qr_text),
        );

    backdrop()
        .child(
            div()
                .w(px(784.))
                .p(px(32.))
                .rounded(px(8.))
                .bg(c(tok(idx::MODAL_BACKGROUND)))
                .shadow_lg()
                .flex()
                .gap(px(64.))
                .items_center()
                .child(form)
                .child(div().w(px(1.)).h(px(300.)).bg(c(tok(idx::MODAL_BACKGROUND))))
                .child(qr),
        )
        .into_any_element()
}

fn qr_code(modules: &[bool], n: usize) -> AnyElement {
    let cell = (160. / n as f32).floor().max(1.);
    let mut col = div().flex().flex_col();
    for y in 0..n {
        let mut row = div().flex();
        for x in 0..n {
            row = row.child(div().size(px(cell)).when(modules[y * n + x], |d| d.bg(gpui::black())));
        }
        col = col.child(row);
    }
    let side = cell * n as f32;
    div()
        .relative()
        .child(col)
        .child(
            div()
                .absolute()
                .top(px(side / 2. - 16.))
                .left(px(side / 2. - 16.))
                .size(px(32.))
                .rounded(px(6.))
                .bg(gpui::white())
                .flex()
                .items_center()
                .justify_center()
                .child(img("icon.png").size(px(28.))),
        )
        .into_any_element()
}
