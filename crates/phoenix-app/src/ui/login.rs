use gpui::{
    AnyElement, App, Context, Entity, Focusable, FontWeight, HighlightStyle, IntoElement, ObjectFit, ParentElement, SharedString,
    StyledText, Styled, Window, div, img, prelude::*, px, rgba,
};
use gpui_component::input::{Input, InputState};

use crate::{
    app::PhoenixApp,
    auth::QrState,
    backend::BackendError,
    theme::{Palette, c, generated::idx, tok, z},
};

pub const ARTWORK: &str = "https://discord.com/assets/d8680b1c1576ecc8.svg";
pub const LOGO: &str = "https://discord.com/assets/131c318dd45b7aa4.svg";
pub const QR_LOGO: &str = "https://discord.com/assets/dd05fd1ea37e7747.png";

const CARD_W: f32 = 784.;
const FORM_W: f32 = 416.;
const QR_W: f32 = 240.;
const QR_BOX: f32 = 176.;
const QR_SIDE: f32 = 160.;

fn label(text: SharedString) -> impl IntoElement {
    div()
        .mb(z(8.))
        .flex()
        .items_start()
        .text_size(z(16.))
        .line_height(z(20.))
        .child(div().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(text))
        .child(div().ml(z(4.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child("*"))
}

fn error_line(error: Option<&str>) -> Option<impl IntoElement> {
    error.map(|e| {
        div()
            .mt(z(4.))
            .flex()
            .items_start()
            .gap(z(4.))
            .child(
                gpui::svg()
                    .path(crate::icons::path("settings/circle-error"))
                    .flex_none()
                    .mt(z(2.))
                    .size(z(12.))
                    .text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))),
            )
            .child(
                div()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))
                    .child(SharedString::from(e.to_owned())),
            )
    })
}

fn field(state: &Entity<InputState>, h: f32, error: bool, trailing: Option<AnyElement>, window: &Window, cx: &App) -> impl IntoElement {
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    let border = if error {
        idx::INPUT_BORDER_ERROR_DEFAULT
    } else if focused {
        idx::INPUT_BORDER_ACTIVE
    } else {
        idx::INPUT_BORDER_DEFAULT
    };
    div()
        .h(z(h))
        .pl(z(12.))
        .pr(z(if trailing.is_some() { 4. } else { 12. }))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(border)))
        .when(focused || error, |d| d.border_2())
        .when(!error && !focused, |d| d.hover(|s| s.border_color(c(tok(idx::INPUT_BORDER_HOVER)))))
        .bg(c(tok(if error { idx::INPUT_BACKGROUND_ERROR_DEFAULT } else { idx::INPUT_BACKGROUND_DEFAULT })))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(16.))
        .text_color(c(tok(if error { idx::INPUT_TEXT_ERROR_DEFAULT } else { idx::INPUT_TEXT_DEFAULT })))
        .child(div().flex_1().min_w_0().child(Input::new(state).appearance(false).bordered(false)))
        .children(trailing)
}

fn text_link(id: &'static str, text: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_LINK)))
        .cursor_pointer()
        .hover(|s| s.underline())
        .child(text)
}

fn backdrop() -> gpui::Div {
    div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::linear_gradient(
            135.,
            gpui::linear_color_stop(c(0x1C0C6B), 0.),
            gpui::linear_color_stop(c(0x0B7FF5), 1.),
        ))
        .child(img(ARTWORK).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover))
        .child(img(LOGO).absolute().top(z(48.)).left(z(48.)).w(z(124.)).h(z(24.)))
}

fn reveal_button(revealed: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .id("login-reveal")
        .size(z(32.))
        .flex_none()
        .rounded(z(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .on_click(cx.listener(|a, _, w, cx| {
            a.login_ui.reveal_password = !a.login_ui.reveal_password;
            let masked = !a.login_ui.reveal_password;
            a.login_password.update(cx, |s, cx| s.set_masked(masked, w, cx));
            cx.notify();
        }))
        .child(
            gpui::svg()
                .path(crate::icons::path(if revealed { "eye-slash" } else { "eye" }))
                .size(z(16.))
                .text_color(c(tok(idx::TEXT_STRONG))),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if app.login_mfa_step && !app.login_with_token {
        return backdrop().child(super::login_mfa::render(app, window, cx)).into_any_element();
    }
    let err: Option<&str> = app.login_error.as_ref().map(AsRef::<str>::as_ref);
    let (err_login, err_password) = match err.and_then(BackendError::field_errors) {
        Some(fields) => fields,
        None => (err, err),
    };
    let header = div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(crate::t!("login.welcome")),
        )
        .child(
            div()
                .mt(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(if app.login_with_token { crate::t!("login.paste_token") } else { crate::t!("login.excited") }),
        );
    let fields = if app.login_with_token {
        div()
            .flex()
            .flex_col()
            .mb(z(20.))
            .child(label(crate::t!("login.token")))
            .child(field(&app.login_token, 44., err.is_some(), None, window, cx))
            .children(error_line(err))
            .child(
                div()
                    .mt(z(8.))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(crate::t!("login.token_note")),
            )
    } else {
        let reveal = reveal_button(app.login_ui.reveal_password, cx);
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(z(20.))
                    .flex()
                    .flex_col()
                    .child(label(crate::t!("login.email")))
                    .child(field(&app.login_email, 44., err_login.is_some(), None, window, cx))
                    .children(error_line(err_login)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(label(crate::t!("login.password")))
                    .child(field(&app.login_password, 40., err_password.is_some(), Some(reveal), window, cx))
                    .children(error_line(err_password)),
            )
            .child(div().mt(z(4.)).mb(z(20.)).flex().child(text_link("login-forgot", crate::t!("login.forgot"))))
    };
    let submit = div()
        .id("login-btn")
        .mb(z(8.))
        .h(z(40.))
        .rounded(z(8.))
        .border_1()
        .border_color(rgba(0xffffff14))
        .bg(c(Palette::BLURPLE))
        .hover(|s| s.bg(c(0x4752C4)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .on_click(cx.listener(|a, _, w, cx| a.submit_login(w, cx)))
        .child(if app.logging_in { SharedString::from("…") } else { crate::t!("login.login") });
    let footer = div()
        .mt(z(4.))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(16.))
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(crate::t!("login.need_account")),
        )
        .child(text_link("login-register", crate::t!("login.register")));
    let toggle = text_link(
        "toggle-token",
        if app.login_with_token { crate::t!("login.with_email") } else { crate::t!("login.with_token") },
    )
    .mt(z(4.))
    .on_click(cx.listener(|a, _, _, cx| {
        a.login_with_token = !a.login_with_token;
        a.login_mfa_step = false;
        a.login_error = None;
        cx.notify();
    }));
    let form = div()
        .w(z(FORM_W))
        .flex_none()
        .flex()
        .flex_col()
        .justify_center()
        .child(header)
        .child(div().mt(z(20.)).flex().flex_col().child(fields).child(submit).child(footer).child(div().flex().child(toggle)));

    backdrop()
        .child(
            div()
                .w(z(CARD_W))
                .p(z(32.))
                .rounded(z(8.))
                .bg(c(tok(idx::MODAL_BACKGROUND)))
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.2),
                    offset: gpui::point(px(0.), z(2.)),
                    blur_radius: z(10.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .flex()
                .gap(z(64.))
                .child(form)
                .child(qr_panel(app, cx)),
        )
        .into_any_element()
}

fn qr_heading(text: SharedString) -> impl IntoElement {
    div()
        .w_full()
        .mb(z(8.))
        .text_size(z(24.))
        .line_height(z(32.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .text_center()
        .child(text)
}

fn qr_body(text: SharedString) -> impl IntoElement {
    let (plain, ranges) = crate::ui::modals::strong_ranges(&text);
    let styled = StyledText::new(plain).with_highlights(
        ranges.into_iter().map(|r| (r, HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })),
    );
    div()
        .w_full()
        .text_size(z(16.))
        .line_height(z(20.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .text_center()
        .child(styled)
}

fn qr_box(inner: AnyElement) -> impl IntoElement {
    div()
        .size(z(QR_BOX))
        .flex_none()
        .mb(z(32.))
        .rounded(z(4.))
        .bg(gpui::white())
        .flex()
        .items_center()
        .justify_center()
        .child(inner)
}

fn qr_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let col = div().w(z(QR_W)).flex_none().flex().flex_col().items_center().justify_center();
    match &app.login_ui.qr {
        QrState::Scanned { name, avatar } => col
            .child(div().mb(z(32.)).child(crate::ui::avatar_ref(crate::model::ImageRef::Url(avatar.clone().into()), 120.)))
            .child(qr_heading(crate::t!("login.check_phone")))
            .child(
                div()
                    .w_full()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .text_center()
                    .child(crate::t!("login.qr_scanned", name = name)),
            )
            .child(
                div().mt(z(16.)).child(
                    text_link("qr-start-over", crate::t!("login.start_over"))
                        .text_color(c(tok(idx::BRAND_500)))
                        .on_click(cx.listener(|a, _, _, cx| crate::auth::retry_qr(a, cx))),
                ),
            ),
        state => {
            let inner = match state {
                QrState::Code { modules, size, .. } => qr_code(modules, *size),
                _ => div().size(z(QR_SIDE)).flex().items_center().justify_center().child(crate::ui::spinner::spinner()).into_any_element(),
            };
            col.child(qr_box(inner))
                .child(qr_heading(crate::t!("login.qr_title")))
                .child(qr_body(crate::t!("login.qr_hint")))
                .child(div().p(z(8.)).flex().justify_center().child(text_link("qr-passkey", crate::t!("login.passkey"))))
        }
    }
}

fn qr_code(modules: &[bool], n: usize) -> AnyElement {
    let modules = modules.to_vec();
    let grid = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let cell = bounds.size.width / n.max(1) as f32;
            for y in 0..n {
                let mut x = 0;
                while x < n {
                    if !modules[y * n + x] {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < n && modules[y * n + x] {
                        x += 1;
                    }
                    let origin = gpui::point(bounds.origin.x + cell * start as f32, bounds.origin.y + cell * y as f32);
                    window.paint_quad(gpui::fill(gpui::Bounds::new(origin, gpui::size(cell * (x - start) as f32, cell)), gpui::black()));
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div()
        .relative()
        .size(z(QR_SIDE))
        .child(grid)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(img(QR_LOGO).size(z(50.))),
        )
        .into_any_element()
}
