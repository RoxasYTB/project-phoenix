use std::collections::HashSet;

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, StyledText, Window,
    div, img, prelude::*, px,
};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

use phoenix_api::models as api;

use crate::{
    app::PhoenixApp,
    commands::{
        ParityRequest,
        state::{ModalSpec, ModalView},
        store::{self, Comp, Emoji, PollView},
    },
    markdown::MentionResolver,
    model::{Id, Message},
    richtext::{self, RBlock},
    theme::{Palette, c, fz, generated::idx, tok, z},
};

struct Names<'a>(&'a PhoenixApp);

impl MentionResolver for Names<'_> {
    fn user(&self, id: u64) -> Option<String> {
        self.0.users.get(&Id(id)).map(|u| u.display_name.to_string())
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.0.channels.get(&Id(id)).map(|c| c.name.to_string())
    }
    fn mention_channel(&self, id: u64) -> Option<crate::markdown::MentionChannel> {
        self.0.resolver().mention_channel(id)
    }
    fn role(&self, id: u64) -> Option<String> {
        self.0.roles.values().flatten().find(|r| r.id == Id(id)).map(|r| r.name.to_string())
    }
}

pub fn markdown(app: &PhoenixApp, text: &str) -> AnyElement {
    markdown_sized(app, text, false, None, None)
}

pub fn markdown_in(app: &PhoenixApp, text: &str, entity: &Entity<PhoenixApp>, msg: Id) -> AnyElement {
    markdown_sized(app, text, false, Some((entity, msg)), None)
}

pub fn markdown_highlighted(
    app: &PhoenixApp,
    text: &str,
    entity: &Entity<PhoenixApp>,
    msg: Id,
    hl: &crate::search_highlight::Highlighter,
) -> AnyElement {
    markdown_sized(app, text, false, Some((entity, msg)), Some(hl))
}

pub fn markdown_md(app: &PhoenixApp, text: &str) -> AnyElement {
    markdown_lines(app, text, false, None, None, Some(20.))
}

pub fn markdown_body_sm(app: &PhoenixApp, text: &str) -> AnyElement {
    markdown_lines(app, text, true, None, None, Some(18.))
}

fn markdown_sized(
    app: &PhoenixApp,
    text: &str,
    reduced: bool,
    ctx: Option<(&Entity<PhoenixApp>, Id)>,
    hl: Option<&crate::search_highlight::Highlighter>,
) -> AnyElement {
    markdown_lines(app, text, reduced, ctx, hl, None)
}

fn markdown_lines(
    app: &PhoenixApp,
    text: &str,
    reduced: bool,
    ctx: Option<(&Entity<PhoenixApp>, Id)>,
    hl: Option<&crate::search_highlight::Highlighter>,
    line: Option<f32>,
) -> AnyElement {
    let k = if reduced { 0.875 } else { 1. };
    let (parsed, mut blocks, _, _, _) = richtext::build(text, &Names(app), app.me.id, &|_| true);
    if let Some(hl) = hl {
        richtext::apply_highlights(&parsed, &mut blocks, hl);
    }
    let mut col = div()
        .flex()
        .flex_col()
        .min_w_0()
        .text_size(fz(16. * k))
        .line_height(fz(line.unwrap_or(22. * k)))
        .text_color(c(Palette::TEXT_NORMAL));
    let n = blocks.len();
    for (bi, b) in blocks.into_iter().enumerate() {
        col = col.child(match b {
            RBlock::Code { code, .. } => div()
                .my(z(4.))
                .p(z(8.))
                .rounded(z(4.))
                .bg(c(Palette::CODE_BG))
                .border_1()
                .border_color(c(Palette::CODE_BORDER))
                .font_family(crate::theme::FONT_MONO())
                .text_size(fz(14.))
                .child(code)
                .into_any_element(),
            RBlock::Quote(t) => div()
                .flex()
                .gap(z(8.))
                .child(div().w(z(4.)).rounded(z(4.)).bg(c(Palette::INTERACTIVE_MUTED)))
                .child(div().flex_1().min_w_0().child(text_el(ctx, bi, t, k)))
                .into_any_element(),
            RBlock::Heading(l, t) => {
                let size = heading_px(l);
                div()
                    .when(bi > 0, |d| d.mt(z(16.)))
                    .when(bi + 1 < n, |d| d.mb(z(8.)))
                    .text_size(fz(size))
                    .line_height(gpui::relative(1.375))
                    .child(text_el(ctx, bi, t, size / 16.))
                    .into_any_element()
            }
            RBlock::Subtext(t) => {
                let size = if reduced { 12. } else { 14. };
                div()
                    .text_size(fz(size))
                    .line_height(fz(size * 1.375))
                    .text_color(crate::theme::c(crate::theme::tok(crate::theme::generated::idx::TEXT_SUBTLE)))
                    .child(text_el(ctx, bi, t, size / 16.))
                    .into_any_element()
            }
            RBlock::ListItem { ordered, depth, text } => div()
                .flex()
                .pl(z(16. * (depth as f32 + 1.)))
                .gap(z(4.))
                .child(match ordered {
                    Some(n) => format!("{n}."),
                    None => "•".into(),
                })
                .child(div().flex_1().min_w_0().child(text_el(ctx, bi, text, k)))
                .into_any_element(),
            RBlock::Text(t) => text_el(ctx, bi, t, k),
        });
    }
    col.into_any_element()
}

fn heading_px(level: u8) -> f32 {
    match level {
        1 => 24.,
        2 => 20.,
        _ => 16.,
    }
}

fn text_el(ctx: Option<(&Entity<PhoenixApp>, Id)>, bi: usize, t: richtext::RichText, k: f32) -> AnyElement {
    match ctx {
        Some((e, msg)) if !t.emoji.is_empty() || t.has_mention() => crate::ui::chat::message_ext::rich_with_emoji_scaled(e, msg, bi, &t, k),
        None if !t.emoji.is_empty() => crate::ui::emoji_text::rich_static(&t, 16. * k, 22. * k),
        _ => div().child(StyledText::new(t.text).with_runs(t.runs)).into_any_element(),
    }
}

fn emoji_el(e: &Emoji, size: f32) -> AnyElement {
    crate::media::emoji::emoji_element(&e.token(), size)
}

#[derive(Clone)]
struct Ctx {
    entity: Entity<PhoenixApp>,
    guild: Option<Id>,
    channel: Id,
    message: Id,
    app: Id,
    flags: u64,
    reduced: bool,
    interactive: bool,
}

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn render(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, cx: &App) -> Option<AnyElement> {
    render_with(app, entity, msg, true, cx)
}

pub fn render_static(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, cx: &App) -> Option<AnyElement> {
    render_with(app, entity, msg, false, cx)
}

fn render_with(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, interactive: bool, cx: &App) -> Option<AnyElement> {
    let parts = store::get(msg.id)?;
    if parts.components.is_empty() {
        return None;
    }
    let v2 = parts.flags & api::FLAG_IS_COMPONENTS_V2 != 0;
    let k = Ctx {
        entity: entity.clone(),
        guild: app.channels.get(&msg.channel).and_then(|c| c.guild),
        channel: msg.channel,
        message: msg.id,
        app: parts.app,
        flags: parts.flags,
        reduced: false,
        interactive,
    };
    let mut col = div().flex().flex_col().min_w_0().gap(z(if v2 { 8. } else { 6. }));
    for (i, comp) in parts.components.iter().enumerate() {
        col = col.child(comp_el(app, &k, comp, i, cx));
    }
    Some(if v2 {
        div().mt(z(2.)).flex().child(col.max_w(z(600.))).into_any_element()
    } else {
        col.mt(z(2.)).into_any_element()
    })
}

fn comp_el(app: &PhoenixApp, k: &Ctx, comp: &Comp, ix: usize, cx: &App) -> AnyElement {
    match comp {
        Comp::ActionRow(r) => {
            let mut row = div().flex().flex_wrap().gap(z(8.));
            for (j, it) in r.components.iter().enumerate() {
                row = row.child(comp_el(app, k, it, ix * 8 + j, cx));
            }
            div().flex().flex_col().child(row).into_any_element()
        }
        Comp::Button(b) => button(app, k, b, ix),
        Comp::Select(s) => select(app, k, s),
        Comp::TextDisplay(d) => markdown_sized(app, &d.content, k.reduced, Some((&k.entity, k.message)), None),
        Comp::Section(s) => section(app, k, s, ix, cx),
        Comp::Thumbnail(th) => thumbnail(th, k.message, ix, cx),
        Comp::MediaGallery(g) => gallery(&g.items, k.message, ix, cx),
        Comp::File(f) => file(f, k.message, ix, cx),
        Comp::Separator(s) => div()
            .when(s.spacing == api::SeparatorSpacing::Large, |d| d.my(z(8.)))
            .when(s.divider, |d| d.h(z(1.)).bg(t(idx::BACKGROUND_MOD_STRONG)))
            .into_any_element(),
        Comp::Container(ct) => container(app, k, ct, ix, cx),
        Comp::TextInput(_) => div().into_any_element(),
        Comp::Unknown(_) => unknown(),
    }
}

fn unknown() -> AnyElement {
    div()
        .p(z(16.))
        .rounded(z(12.))
        .border_1()
        .border_color(t(idx::STATUS_WARNING))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t(idx::TEXT_SUBTLE))
        .child(crate::t!("components.unsupported"))
        .into_any_element()
}

fn section(app: &PhoenixApp, k: &Ctx, s: &api::Section, ix: usize, cx: &App) -> AnyElement {
    let button = matches!(s.accessory.as_deref(), Some(Comp::Button(_)));
    let mut texts = div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).when(button, |d| d.justify_center());
    for (j, c) in s.components.iter().enumerate() {
        texts = texts.child(comp_el(app, k, c, ix * 8 + j, cx));
    }
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(
            div().flex().gap(z(12.)).justify_between().child(texts).when_some(s.accessory.as_deref(), |d, a| {
                d.child(div().flex().items_start().min_w_0().max_w(gpui::relative(0.5)).flex_none().child(comp_el(app, k, a, ix * 8 + 7, cx)))
            }),
        )
        .into_any_element()
}

fn container(app: &PhoenixApp, k: &Ctx, ct: &api::Container, ix: usize, cx: &App) -> AnyElement {
    if ct.components.is_empty() {
        return div().into_any_element();
    }
    let k = &Ctx { reduced: true, ..k.clone() };
    let mut card = div()
        .relative()
        .flex()
        .flex_col()
        .min_w_0()
        .gap(z(8.))
        .p(z(16.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE));
    for (j, ch) in ct.components.iter().enumerate() {
        card = card.child(comp_el(app, k, ch, ix * 16 + j, cx));
    }
    if let Some(a) = ct.accent_color {
        card = card.child(div().absolute().top_0().left_0().bottom_0().w(z(4.)).bg(c(a)));
    }
    let key: SharedString = format!("component-spoiler-{}-{ix}", k.message.0).into();
    if ct.spoiler && !crate::media::is_revealed(&key, cx) {
        card = card.child(spoiler_cover(key, true).top(z(-1.)).left(z(-1.)).right(z(-1.)).bottom(z(-1.)));
    }
    card.into_any_element()
}

fn spoiler_overlay(key: SharedString) -> AnyElement {
    spoiler_cover(key, false).inset_0().into_any_element()
}

fn spoiler_cover(key: SharedString, veil: bool) -> gpui::Stateful<gpui::Div> {
    let group: SharedString = format!("{key}-cover").into();
    div()
        .id(gpui::ElementId::Name(key.clone()))
        .absolute()
        .cursor_pointer()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(t(idx::SPOILER_BACKGROUND_COLOR_HIDDEN))
        .hover(|s| s.bg(t(idx::SPOILER_BACKGROUND_COLOR_HIDDEN_HOVER)))
        .shadow(vec![gpui::BoxShadow {
            color: t(idx::SPOILER_CONTAINER_BOX_SHADOW_COLOR),
            offset: gpui::point(z(0.5), z(0.5)),
            blur_radius: z(1.),
            spread_radius: z(1.),
            inset: false,
        }])
        .group(group.clone())
        .flex()
        .items_center()
        .justify_center()
        .when(veil, |d| d.child(blurred_veil(t(idx::BACKGROUND_SURFACE_HIGH), 44. * crate::theme::zoom(), 8. * crate::theme::zoom())))
        .child(
            div()
                .px(z(12.))
                .py(z(8.))
                .rounded(z(20.))
                .bg(t(idx::SPOILER_WARNING_BACKGROUND_COLOR))
                .text_size(z(15.))
                .line_height(z(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t(idx::SPOILER_WARNING_TEXT_COLOR))
                .group_hover(group, |s| s.bg(t(idx::SPOILER_WARNING_BACKGROUND_COLOR_HOVER)).text_color(t(idx::SPOILER_WARNING_TEXT_COLOR_HOVER)))
                .child(crate::t!("chat.spoiler")),
        )
        .on_click(move |_, _, cx| crate::media::reveal(key.clone(), cx))
}

fn erf(x: f32) -> f32 {
    let s = x.signum();
    let x = x.abs();
    let t = 1. / (1. + 0.327_591_1 * x);
    let y = 1. - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_7) * t - 0.284_496_74) * t + 0.254_829_6) * t * (-x * x).exp();
    s * y
}

pub fn blur_coverage(x: f32, len: f32, sigma: f32) -> f32 {
    let k = 1. / (sigma * std::f32::consts::SQRT_2);
    0.5 * (erf((len - x) * k) + erf(x * k))
}

fn blurred_veil(color: gpui::Hsla, sigma: f32, radius: f32) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            if w <= 0. || h <= 0. {
                return;
            }
            let cell = (w.max(h) / 64.).max(4.);
            let (nx, ny) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
            let (cw, chh) = (w / nx as f32, h / ny as f32);
            let cols: Vec<f32> = (0..nx).map(|i| blur_coverage((i as f32 + 0.5) * cw, w, sigma)).collect();
            for j in 0..ny {
                let cy = (j as f32 + 0.5) * chh;
                let ay = blur_coverage(cy, h, sigma);
                for (i, ax) in cols.iter().enumerate() {
                    let cx = (i as f32 + 0.5) * cw;
                    let (dx, dy) = ((radius - cx).max(cx - (w - radius)).max(0.), (radius - cy).max(cy - (h - radius)).max(0.));
                    if dx * dx + dy * dy > radius * radius {
                        continue;
                    }
                    let mut c = color;
                    c.a *= ax * ay;
                    let o = gpui::point(b.origin.x + gpui::px(i as f32 * cw), b.origin.y + gpui::px(j as f32 * chh));
                    window.paint_quad(gpui::fill(gpui::Bounds::new(o, gpui::size(gpui::px(cw), gpui::px(chh))), c));
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn button_tokens(style: api::ButtonStyle) -> [usize; 5] {
    use api::ButtonStyle as S;
    match style {
        S::Primary | S::Premium => [
            idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT,
            idx::CONTROL_PRIMARY_BACKGROUND_HOVER,
            idx::CONTROL_PRIMARY_BACKGROUND_ACTIVE,
            idx::CONTROL_PRIMARY_BORDER_DEFAULT,
            idx::CONTROL_PRIMARY_TEXT_DEFAULT,
        ],
        S::Success => [
            idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT,
            idx::CONTROL_CONNECTED_BACKGROUND_HOVER,
            idx::CONTROL_CONNECTED_BACKGROUND_ACTIVE,
            idx::CONTROL_CONNECTED_BORDER_DEFAULT,
            idx::WHITE,
        ],
        S::Danger => [
            idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT,
            idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER,
            idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_ACTIVE,
            idx::CONTROL_CRITICAL_PRIMARY_BORDER_DEFAULT,
            idx::WHITE,
        ],
        _ => [
            idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT,
            idx::CONTROL_SECONDARY_BACKGROUND_HOVER,
            idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE,
            idx::BORDER_MUTED,
            idx::CONTROL_SECONDARY_TEXT_DEFAULT,
        ],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ButtonAction {
    Open(String),
    Store,
    Interact(String),
    None,
}

pub fn button_action(b: &api::Button) -> ButtonAction {
    match b.style {
        api::ButtonStyle::Link => b.url.clone().filter(|u| !u.is_empty()).map_or(ButtonAction::None, ButtonAction::Open),
        api::ButtonStyle::Premium => ButtonAction::Store,
        _ => b.custom_id.clone().map_or(ButtonAction::None, ButtonAction::Interact),
    }
}

fn button(app: &PhoenixApp, k: &Ctx, b: &api::Button, ix: usize) -> AnyElement {
    let [bg, hover, active, border, fg] = button_tokens(b.style);
    let action = button_action(b);
    let premium = b.style == api::ButtonStyle::Premium;
    let label: Option<SharedString> = if premium {
        Some(crate::t!("components.product_unavailable"))
    } else {
        b.label.clone().filter(|l| !l.is_empty()).map(SharedString::from)
    };
    let pending = match &action {
        ButtonAction::Interact(id) => app.parity.pending.contains(&(k.message, id.clone())),
        _ => false,
    };
    let disabled = b.disabled || premium;
    let has_label = label.is_some();
    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .min_w(z(32.))
        .overflow_hidden()
        .when(pending, |d| d.invisible())
        .when(premium, |d| {
            d.child(div().flex().items_center().mr(z(4.)).child(icon_svg("shop", 16., gpui::white())))
        })
        .when_some(b.emoji.as_ref(), |d, e| {
            d.child(div().flex_none().when(has_label, |d| d.mr(z(4.))).child(crate::media::emoji::emoji_element(&e.token(), 19.25)))
        })
        .when_some(label, |d, l| d.child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(l)))
        .when(b.style == api::ButtonStyle::Link, |d| d.child(div().flex_none().ml(z(8.)).child(icon_svg("viewer/open", 16., t(fg)))));
    let mut el = div()
        .id(("component-button", (k.message.0 << 8) ^ ix as u64))
        .relative()
        .h(z(32.))
        .min_w(z(60.))
        .px(z(11.))
        .py(z(3.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(border))
        .bg(t(bg))
        .flex()
        .items_center()
        .justify_center()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t(fg))
        .child(content)
        .when(pending, |d| d.child(div().absolute().inset_0().flex().items_center().justify_center().child("•••")));
    if disabled || pending {
        return el.opacity(0.5).cursor_not_allowed().into_any_element();
    }
    el = el.cursor_pointer().hover(move |s| s.bg(t(hover))).active(move |s| s.bg(t(active)));
    if !k.interactive {
        return el.into_any_element();
    }
    let k = k.clone();
    el.on_click(move |_, w, cx: &mut App| match &action {
        ButtonAction::Open(u) => k.entity.clone().update(cx, |a, cx| crate::ui::modals::leave_link::click(a, "", u, w, cx)),
        ButtonAction::Interact(id) => {
            let (k, id) = (k.clone(), id.clone());
            k.entity.clone().update(cx, |a, cx| a.send_component(&k, id, 2, None, cx));
        }
        ButtonAction::Store | ButtonAction::None => {}
    })
    .into_any_element()
}

fn icon_svg(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(color)
}

fn auto_options(app: &PhoenixApp, kind: api::SelectKind, guild: Option<Id>) -> Vec<api::SelectOption> {
    use api::SelectKind as K;
    let opt = |label: String, value: u64| api::SelectOption { label, value: value.to_string(), ..Default::default() };
    let mut out = Vec::new();
    if matches!(kind, K::User | K::Mentionable) {
        let members: Vec<Id> = match guild {
            Some(g) => app.members.get(&g).map(|m| m.iter().map(|m| m.user).collect()).unwrap_or_default(),
            None => app.users.keys().copied().collect(),
        };
        out.extend(members.iter().filter_map(|u| app.users.get(u)).map(|u| opt(u.display_name.to_string(), u.id.0)));
    }
    if matches!(kind, K::Role | K::Mentionable)
        && let Some(g) = guild
    {
        out.extend(app.roles.get(&g).into_iter().flatten().map(|r| opt(format!("@{}", r.name), r.id.0)));
    }
    if kind == K::Channel
        && let Some(g) = guild
    {
        out.extend(
            app.guild_channels
                .get(&g)
                .into_iter()
                .flatten()
                .filter_map(|c| app.channels.get(c))
                .filter(|c| c.kind != crate::model::ChannelKind::Category)
                .map(|c| opt(format!("#{}", c.name), c.id.0)),
        );
    }
    out.truncate(25);
    out
}

fn select(app: &PhoenixApp, k: &Ctx, s: &api::SelectMenu) -> AnyElement {
    let kind = s.kind;
    let key = (k.message, s.custom_id.clone());
    let open = app.parity.open_select.as_ref() == Some(&key);
    let options: Vec<api::SelectOption> =
        if kind == api::SelectKind::String { s.options.clone() } else { auto_options(app, kind, k.guild) };
    let picked: Vec<String> = app.parity.select_values.get(&key).cloned().unwrap_or_else(|| {
        let mut v: Vec<String> = options.iter().filter(|o| o.default).map(|o| o.value.clone()).collect();
        v.extend(s.default_values.iter().map(|d| d.id.0.to_string()));
        v
    });
    let shown: Vec<&api::SelectOption> = options.iter().filter(|o| picked.contains(&o.value)).collect();
    let label: SharedString = if shown.is_empty() {
        s.placeholder.clone().map(SharedString::from).unwrap_or_else(|| crate::t!("select.placeholder"))
    } else {
        shown.iter().map(|o| o.label.as_str()).collect::<Vec<_>>().join(", ").into()
    };
    let (max, disabled) = (s.max_values, s.disabled);
    let header = div()
        .id(("select", k.message.0 ^ hash(&s.custom_id)))
        .h(z(40.))
        .w_full()
        .max_w(z(400.))
        .pl(z(12.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(t(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT }))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(t(if shown.is_empty() { idx::TEXT_MUTED } else { idx::TEXT_STRONG }))
                .child(label),
        )
        .child(
            div()
                .w(z(36.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .pr(z(4.))
                .child(icon_svg("chevron", 20., t(idx::TEXT_STRONG)).when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))),
        );
    let header = if disabled {
        header.opacity(0.5).cursor_not_allowed()
    } else {
        let hover = header.cursor_pointer().hover(|s| s.border_color(t(idx::INPUT_BORDER_HOVER)));
        if k.interactive {
            let (e, key2, kk) = (k.entity.clone(), key.clone(), k.clone());
            hover.on_click(move |_, _, cx: &mut App| {
                let (key, kk) = (key2.clone(), kk.clone());
                e.update(cx, |a, cx| a.toggle_select(&kk, key, kind.type_id(), max, cx));
            })
        } else {
            hover
        }
    };
    let mut col = div().flex().flex_col().w_full().max_w(z(400.)).child(header);
    if open && k.interactive {
        let mut menu = div()
            .id(("select-menu", k.message.0 ^ hash(&s.custom_id)))
            .mt(z(4.))
            .w_full()
            .max_h(z(5. * 40. + 8.))
            .overflow_y_scroll()
            .rounded(z(8.))
            .bg(t(idx::BACKGROUND_SURFACE_HIGHER))
            .border_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .py(z(4.));
        if options.is_empty() {
            menu = menu.child(div().p(z(12.)).text_color(t(idx::TEXT_MUTED)).child(crate::t!("select.none")));
        }
        for (i, o) in options.into_iter().enumerate() {
            let on = picked.contains(&o.value);
            let (e, kk, key, value) = (k.entity.clone(), k.clone(), key.clone(), o.value.clone());
            menu = menu.child(
                div()
                    .id(("select-opt", i as u64))
                    .px(z(12.))
                    .py(z(8.))
                    .mx(z(4.))
                    .rounded(z(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .when_some(o.emoji.as_ref(), |d, em| d.child(crate::media::emoji::emoji_element(&em.token(), 20.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().text_size(z(16.)).line_height(z(20.)).text_color(t(idx::TEXT_STRONG)).child(o.label.clone()))
                            .when_some(o.description.clone(), |d, ds| {
                                d.child(div().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_MUTED)).child(ds))
                            }),
                    )
                    .when(max > 1, |d| d.child(checkbox(on)))
                    .when(max <= 1 && on, |d| d.child(icon_svg("check", 20., t(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT))))
                    .on_click(move |_, _, cx: &mut App| {
                        let (kk, key, value) = (kk.clone(), key.clone(), value.clone());
                        e.update(cx, |a, cx| a.pick_option(&kk, key, kind.type_id(), value, max, cx));
                    }),
            );
        }
        col = col.child(menu);
    }
    col.into_any_element()
}

fn as_attachment(m: &api::UnfurledMedia, name: Option<&str>, size: u64, spoiler: bool) -> crate::model::Attachment {
    use crate::model::AttachmentKind as K;
    let ct = m.content_type.as_deref().unwrap_or("");
    let sized = m.size().is_some();
    let kind = if ct.starts_with("video/") && sized {
        K::Video
    } else if (ct.starts_with("image/") || ct.is_empty()) && sized {
        K::Image
    } else if ct.starts_with("audio/") {
        K::Audio
    } else {
        K::File
    };
    let base = name.map(str::to_owned).unwrap_or_else(|| {
        let path = m.url.split(['?', '#']).next().unwrap_or_default();
        path.rsplit('/').next().unwrap_or("file").to_owned()
    });
    let (w, h) = m.size().unwrap_or((0, 0));
    crate::model::Attachment {
        filename: base.into(),
        url: if matches!(kind, K::Image | K::Video) { m.fetch_url().to_owned() } else { m.url.clone() }.into(),
        width: w,
        height: h,
        is_image: kind == K::Image,
        size,
        kind,
        original: m.url.clone().into(),
        spoiler,
        description: None,
        voice: None,
    }
}

fn thumbnail(th: &api::Thumbnail, msg: Id, ix: usize, cx: &App) -> AnyElement {
    let a = as_attachment(&th.media, None, 0, th.spoiler);
    let key: SharedString = a.url.clone();
    if th.spoiler && !crate::media::is_revealed(&key, cx) {
        return div()
            .id(("thumb-spoiler", (msg.0 << 8) ^ ix as u64))
            .relative()
            .size(z(85.))
            .rounded(z(8.))
            .overflow_hidden()
            .child(spoiler_overlay(key))
            .into_any_element();
    }
    let viewer = crate::media::ViewerImage {
        url: crate::media::resized(&a.url, a.width.min(2048), a.height.min(2048)),
        original: a.original.clone(),
        filename: a.filename.clone(),
        width: a.width,
        height: a.height,
        author: crate::media::current_author(),
        video: None,
    };
    let image = a.kind == crate::model::AttachmentKind::Image;
    div()
        .id(("thumb", (msg.0 << 8) ^ ix as u64))
        .size(z(85.))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_MOD_NORMAL))
        .when(image, |d| {
            d.cursor_pointer()
                .child(img(crate::media::resized(&a.url, 170, 170)).size_full().object_fit(gpui::ObjectFit::Cover))
                .on_click(move |_, _, cx| crate::media::open_viewer(viewer.clone(), cx))
        })
        .into_any_element()
}

fn gallery(items: &[api::GalleryItem], msg: Id, ix: usize, cx: &App) -> AnyElement {
    let atts: Vec<crate::model::Attachment> = items.iter().map(|i| as_attachment(&i.media, None, 0, i.spoiler)).collect();
    match atts.as_slice() {
        [] => div().into_any_element(),
        [a] if matches!(a.kind, crate::model::AttachmentKind::Image | crate::model::AttachmentKind::Video) => {
            let size = crate::media::fit(a.width, a.height, 600., 350.);
            if a.spoiler
                && let Some(cover) = crate::ui::chat::spoiler_cover(a.url.clone(), &a.url, a.kind == crate::model::AttachmentKind::Video, size, cx)
            {
                return cover;
            }
            if a.kind == crate::model::AttachmentKind::Video {
                return crate::ui::chat::attach::video_card(a, size, cx);
            }
            div()
                .id(("gallery", (msg.0 << 8) ^ ix as u64))
                .pb(z(2.4))
                .child(
                    div()
                        .rounded(z(8.))
                        .overflow_hidden()
                        .child(crate::ui::chat::media_image_with(&a.url, a.original.clone(), a.filename.clone(), (a.width, a.height), size, false)),
                )
                .into_any_element()
        }
        many => {
            let refs: Vec<&crate::model::Attachment> = many.iter().collect();
            if refs.len() >= 2 {
                div().w(z(600.)).max_w_full().child(crate::ui::chat::mosaic::grid(&refs, 600., cx)).into_any_element()
            } else {
                crate::ui::chat::attachment(refs[0], cx)
            }
        }
    }
}

fn file(f: &api::FileComponent, msg: Id, ix: usize, cx: &App) -> AnyElement {
    let a = as_attachment(&f.file, Some(f.file_name().as_str()), f.size.unwrap_or(0), false);
    let key: SharedString = format!("component-file-{}-{ix}", msg.0).into();
    let card = crate::ui::chat::attach::file_card(&a);
    if f.spoiler && !crate::media::is_revealed(&key, cx) {
        return div().relative().max_w_full().child(card).child(spoiler_overlay(key)).into_any_element();
    }
    div().max_w_full().child(card).into_any_element()
}

fn checkbox(on: bool) -> AnyElement {
    div()
        .size(px(20.))
        .flex_none()
        .rounded(px(3.))
        .when(!on, |d| d.border_2().border_color(c(tok(idx::TEXT_DEFAULT))))
        .when(on, |d| d.bg(c(tok(idx::BRAND_500))))
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(px(17.)).text_color(gpui::white())))
        .into_any_element()
}

pub fn circle_check(size: f32, color: gpui::Hsla, secondary: gpui::Hsla) -> gpui::Div {
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .child(gpui::svg().absolute().top_0().left_0().path(crate::icons::path("circle-check-bg")).size(px(size)).text_color(secondary))
        .child(gpui::svg().absolute().top_0().left_0().path(crate::icons::path("circle-check")).size(px(size)).text_color(color))
}

pub fn voted_check(size: f32, victor: bool, expired: bool) -> gpui::Div {
    let (fg, bg) = match (expired, victor) {
        (true, true) => (c(tok(idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT)), gpui::white()),
        (true, false) => (c(tok(idx::ICON_STRONG)), c(tok(idx::BACKGROUND_MOD_MUTED))),
        _ => (c(tok(idx::BRAND_500)), gpui::white()),
    };
    circle_check(size, fg, bg)
}

fn radio(on: bool) -> AnyElement {
    div()
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(20.))
                .rounded_full()
                .border_2()
                .border_color(c(tok(idx::TEXT_DEFAULT)))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.child(div().size(px(10.)).rounded_full().bg(c(tok(idx::CONTROL_BRAND_FOREGROUND))))),
        )
        .into_any_element()
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

fn votes_text(n: u32) -> String {
    let one = if crate::i18n::locale() == "fr" { n < 2 } else { n == 1 };
    if one { crate::t!("poll.vote_one", n = n).to_string() } else { crate::t!("poll.votes", n = n).to_string() }
}

fn time_left(secs: i64) -> String {
    let one = |n: i64| if crate::i18n::locale() == "fr" { n < 2 } else { n == 1 };
    match secs {
        s if s >= 86_400 => crate::t!("poll.days_left", n = s / 86_400).to_string(),
        s if s >= 3_600 && one(s / 3_600) => crate::t!("poll.hours_left_one", n = s / 3_600).to_string(),
        s if s >= 3_600 => crate::t!("poll.hours_left", n = s / 3_600).to_string(),
        s if s >= 60 => crate::t!("poll.minutes_left", n = s / 60).to_string(),
        _ => crate::t!("poll.lt_minute").to_string(),
    }
}

pub fn percent(count: u32, total: u32) -> u32 {
    if total == 0 { 0 } else { ((count as f64 * 100.) / total as f64).round() as u32 }
}

fn answer_style(expired: bool, leader: bool, self_vote: bool) -> (Option<usize>, usize) {
    match (expired, leader, self_vote) {
        (true, true, _) => (Some(idx::STATUS_POSITIVE), idx::POLLS_VICTOR_FILL),
        (false, _, true) => (Some(idx::CONTROL_BRAND_FOREGROUND), idx::POLLS_VOTED_FILL),
        _ => (None, idx::INTERACTIVE_BACKGROUND_ACTIVE),
    }
}

pub fn render_poll(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> Option<AnyElement> {
    let poll = store::get(msg.id)?.poll.clone()?;
    let now = chrono::Utc::now().timestamp();
    let expired = poll.ended(now);
    let voted = poll.voted();
    let show_votes = app.parity.poll_show_results.contains(&msg.id);
    let results = voted || expired || show_votes;
    let total = poll.total();
    let choice = app.parity.poll_choice.get(&msg.id).cloned().unwrap_or_default();
    let (ch, id) = (msg.channel, msg.id);
    let top = poll.answers.iter().map(|a| a.count).max().unwrap_or(0);

    let mut answers = div().mt(px(8.)).mb(px(16.)).flex().flex_col().gap(px(8.));
    for a in &poll.answers {
        let pct = percent(a.count, total);
        let ticked = choice.contains(&a.id);
        let leader = a.count >= top && a.count != 0;
        let self_vote = voted && a.me;
        let (style_border, fill) = answer_style(expired, leader, self_vote);
        let e = entity.clone();
        let aid = a.id;
        let multi = poll.multi;
        let gid: SharedString = format!("poll-answer-{}-{}", id.0, a.id).into();
        let outline = if !results && ticked { Some(idx::CONTROL_BRAND_FOREGROUND) } else { style_border };
        let mut inner = div()
            .relative()
            .min_h(px(50.))
            .px(px(16.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(crate::theme::generated::global::RADIUS_SM))
            .overflow_hidden()
            .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
            .text_color(c(tok(idx::TEXT_DEFAULT)));
        if results {
            inner = inner.child(div().absolute().top_0().left_0().h_full().w(gpui::relative(pct as f32 / 100.)).bg(c(tok(fill))));
        }
        inner = inner
            .when_some(a.emoji.as_ref(), |d, em| d.child(div().mr(px(2.)).flex_none().child(emoji_el(em, 24.))))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::ui::emoji_text::inline_markdown(app, &a.text, c(tok(idx::TEXT_DEFAULT)), FontWeight::SEMIBOLD, 14., false)),
            )
            .when(results, |d| {
                d.child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .whitespace_nowrap()
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(tok(idx::TEXT_STRONG)))
                                .child(votes_text(a.count)),
                        )
                        .child(div().text_size(px(16.)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).child(format!("{pct}%"))),
                )
            })
            .when(results && self_vote, |d| d.child(voted_check(24., leader && expired, expired)))
            .when(!results, |d| d.child(if poll.multi { checkbox(ticked) } else { radio(ticked) }));
        let mut row = div().id(gpui::ElementId::Name(gid.clone())).group(gid.clone()).relative().child(inner).child(
            div()
                .absolute()
                .top(px(-1.))
                .left(px(-1.))
                .right(px(-1.))
                .bottom(px(-1.))
                .rounded(px(crate::theme::generated::global::RADIUS_SM + 1.))
                .border_1()
                .border_color(outline.map(|o| c(tok(o))).unwrap_or_else(gpui::transparent_black))
                .when(!results && !ticked, |d| d.group_hover(gid.clone(), |s| s.border_color(c(tok(idx::BORDER_STRONG))))),
        );
        if !results {
            row = row.cursor_pointer().on_click(move |_, _, cx: &mut App| {
                e.update(cx, |a, cx| a.tick_poll_answer(id, aid, multi, cx));
            });
        }
        answers = answers.child(row);
    }

    let muted = c(tok(idx::TEXT_MUTED));
    let label = if expired { crate::t!("poll.closed").to_string() } else { poll.expiry.map(|e| time_left(e - now)).unwrap_or_default() };
    let details = div()
        .mr_auto()
        .flex()
        .whitespace_nowrap()
        .text_size(px(14.))
        .line_height(px(18.))
        .child(div().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(votes_text(total)))
        .when(!label.is_empty(), |d| d.child(div().mx(px(8.)).text_color(muted).child("\u{2219}")).child(div().text_color(muted).child(label)));
    let tertiary = (!expired && !voted && !show_votes).then(|| {
        let e = entity.clone();
        div()
            .id(("poll-show-results", id.0))
            .cursor_pointer()
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::TEXT_STRONG)))
            .hover(|s| s.underline())
            .child(crate::t!("poll.show_results"))
            .on_click(move |_, _, cx: &mut App| e.update(cx, |a, cx| a.poll_show_results(ch, id, true, cx)))
    });
    use crate::ui::controls::{BtnKind, button_sm};
    let e = entity.clone();
    let primary: Option<AnyElement> = if expired {
        None
    } else if voted {
        Some(
            button_sm(("poll-remove", id.0), crate::t!("poll.remove_vote"), BtnKind::Secondary)
                .on_click(move |_, _, cx: &mut App| e.update(cx, |a, cx| a.vote_poll(ch, id, Vec::new(), cx)))
                .into_any_element(),
        )
    } else if show_votes {
        Some(
            button_sm(("poll-back", id.0), crate::t!("poll.back_to_vote"), BtnKind::Secondary)
                .on_click(move |_, _, cx: &mut App| e.update(cx, |a, cx| a.poll_show_results(ch, id, false, cx)))
                .into_any_element(),
        )
    } else {
        let can_vote = !choice.is_empty();
        Some(
            button_sm(("poll-vote", id.0), crate::t!("poll.vote"), BtnKind::Primary)
                .when(!can_vote, |b| b.opacity(0.5).cursor_not_allowed())
                .when(can_vote, |b| {
                    b.on_click(move |_, _, cx: &mut App| {
                        e.update(cx, |a, cx| {
                            let mut picks: Vec<u32> = a.parity.poll_choice.get(&id).into_iter().flatten().copied().collect();
                            picks.sort_unstable();
                            a.vote_poll(ch, id, picks, cx)
                        })
                    })
                })
                .into_any_element(),
        )
    };

    let prompt = (!expired).then(|| if poll.multi { crate::t!("poll.multi") } else { crate::t!("poll.single") });
    Some(
        div()
            .my(px(4.))
            .w_full()
            .max_w(px(472.))
            .min_w(px(302.))
            .p(px(16.))
            .rounded(px(crate::theme::generated::global::RADIUS_SM))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .flex()
            .flex_col()
            .child(
                div()
                    .pr(px(32.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(tok(idx::TEXT_STRONG)))
                            .child(crate::ui::emoji_text::inline_markdown(app, &poll.question, c(tok(idx::TEXT_STRONG)), FontWeight::MEDIUM, 16., false)),
                    )
                    .when_some(prompt, |d, p| d.child(div().mt(px(4.)).text_size(px(14.)).line_height(px(18.)).text_color(muted).child(p))),
            )
            .child(answers)
            .child(
                div()
                    .min_h(px(crate::theme::generated::global::CUSTOM_BUTTON_BUTTON_MD_HEIGHT))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(16.))
                    .child(details)
                    .children(tertiary)
                    .children(primary),
            )
            .into_any_element(),
    )
}

fn pill_button(id: &'static str, label: SharedString, primary: bool, enabled: bool) -> gpui::Stateful<gpui::Div> {
    let (bg, hover) = if primary { (0x5865F2, 0x4752C4) } else { (0x4E5058, 0x6D6F78) };
    div()
        .id(id)
        .h(px(32.))
        .px(px(16.))
        .rounded(px(8.))
        .bg(c(bg))
        .flex()
        .items_center()
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .child(label)
        .when(enabled, move |d| d.cursor_pointer().hover(move |s| s.bg(c(hover))))
        .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed())
}

impl PhoenixApp {
    fn send_component(&mut self, k: &Ctx, custom_id: String, kind: u8, values: Option<Vec<String>>, cx: &mut gpui::Context<Self>) {
        self.parity.pending.insert((k.message, custom_id.clone()));
        self.parity.last_target = Some((k.guild, k.channel));
        let key = (k.message, custom_id.clone());
        let req = ParityRequest::Component {
            guild: k.guild,
            channel: k.channel,
            message: k.message,
            app: k.app,
            flags: k.flags,
            custom_id,
            kind,
            values,
        };
        let (ch, msg) = (k.channel, k.message);
        self.refresh_message(ch, msg);
        self.parity_request(req, cx, move |a, res, cx| {
            a.parity.pending.remove(&key);
            a.refresh_message(ch, msg);
            if let Err(e) = res {
                a.parity_toast(crate::t!("interaction.failed", e = e.0).to_string(), cx);
            }
        });
    }

    fn toggle_select(&mut self, k: &Ctx, key: (Id, String), kind: u8, max: u32, cx: &mut gpui::Context<Self>) {
        let was_open = self.parity.open_select.as_ref() == Some(&key);
        self.parity.open_select = if was_open { None } else { Some(key.clone()) };
        if was_open
            && max > 1
            && let Some(v) = self.parity.select_values.get(&key).cloned()
        {
            self.send_component(k, key.1.clone(), kind, Some(v), cx);
        }
        self.refresh_message(k.channel, k.message);
        cx.notify();
    }

    fn pick_option(&mut self, k: &Ctx, key: (Id, String), kind: u8, value: String, max: u32, cx: &mut gpui::Context<Self>) {
        if max <= 1 {
            self.parity.select_values.insert(key.clone(), vec![value.clone()]);
            self.parity.open_select = None;
            self.send_component(k, key.1, kind, Some(vec![value]), cx);
        } else {
            let v = self.parity.select_values.entry(key).or_default();
            if let Some(p) = v.iter().position(|x| *x == value) {
                v.remove(p);
            } else if (v.len() as u32) < max {
                v.push(value);
            }
        }
        self.refresh_message(k.channel, k.message);
        cx.notify();
    }

    fn poll_show_results(&mut self, channel: Id, msg: Id, on: bool, cx: &mut gpui::Context<Self>) {
        if on {
            self.parity.poll_show_results.insert(msg);
        } else {
            self.parity.poll_show_results.remove(&msg);
        }
        self.refresh_message(channel, msg);
        cx.notify();
    }

    fn tick_poll_answer(&mut self, msg: Id, answer: u32, multi: bool, cx: &mut gpui::Context<Self>) {
        let set = self.parity.poll_choice.entry(msg).or_default();
        if multi {
            if !set.remove(&answer) {
                set.insert(answer);
            }
        } else {
            *set = HashSet::from([answer]);
        }
        if let Some(ch) = self.selected_channel {
            self.refresh_message(ch, msg);
        }
        cx.notify();
    }

    pub fn vote_poll(&mut self, channel: Id, msg: Id, answers: Vec<u32>, cx: &mut gpui::Context<Self>) {
        let before = store::get(msg).and_then(|p| p.poll.clone());
        store::update_poll(msg, |p| apply_own_vote(p, &answers));
        self.parity.poll_choice.remove(&msg);
        self.refresh_message(channel, msg);
        self.parity_request(ParityRequest::VotePoll { channel, message: msg, answers }, cx, move |a, res, cx| {
            if let Err(e) = res {
                if let Some(old) = before {
                    store::update_poll(msg, |p| *p = old);
                }
                a.refresh_message(channel, msg);
                a.parity_toast(crate::t!("poll.vote_failed", e = e.0).to_string(), cx);
            }
        });
        cx.notify();
    }
}

pub fn apply_own_vote(p: &mut PollView, answers: &[u32]) {
    let old: Vec<u32> = p.answers.iter().filter(|a| a.me).map(|a| a.id).collect();
    for id in old {
        p.apply_vote(id, false, true);
    }
    for id in answers {
        p.apply_vote(*id, true, true);
    }
}

pub enum Field {
    Line(Entity<InputState>),
    Area(Entity<TextareaState>),
}

impl Field {
    pub fn value(&self, cx: &App) -> String {
        match self {
            Field::Line(i) => i.read(cx).value().to_string(),
            Field::Area(i) => i.read(cx).value().to_string(),
        }
    }
}

fn build_modal(spec: ModalSpec, window: &mut Window, cx: &mut gpui::Context<PhoenixApp>) -> ModalView {
    let inputs = spec
        .inputs
        .iter()
        .map(|f| {
            let ph = f.placeholder.clone().unwrap_or_default();
            let val = f.value.clone().unwrap_or_default();
            if f.style == 2 {
                Field::Area(cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8).placeholder(ph).default_value(val)))
            } else {
                Field::Line(cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(val)))
            }
        })
        .collect::<Vec<_>>();
    if let Some(Field::Line(i)) = inputs.first() {
        i.update(cx, |s, cx| s.focus(window, cx));
    }
    ModalView { spec, inputs, sending: false, error: None }
}

impl PhoenixApp {
    fn submit_modal(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(m) = self.parity.modal.as_mut() else { return };
        let mut fields = Vec::new();
        for (spec, f) in m.spec.inputs.iter().zip(&m.inputs) {
            let v = f.value(cx);
            if spec.required && v.trim().is_empty() {
                m.error = Some(crate::t!("form.required", label = spec.label));
                cx.notify();
                return;
            }
            if let Some(min) = spec.min
                && !v.is_empty()
                && (v.chars().count() as u32) < min
            {
                m.error = Some(crate::t!("form.min_chars", label = spec.label, min = min));
                cx.notify();
                return;
            }
            fields.push((spec.custom_id.clone(), v));
        }
        let target = m.spec.channel.map(|c| (m.spec.guild, c)).or(self.parity.last_target);
        let Some((guild, channel)) = target else {
            m.error = Some(crate::t!("form.no_channel"));
            cx.notify();
            return;
        };
        let guild = guild.or_else(|| self.channels.get(&channel).and_then(|c| c.guild));
        m.sending = true;
        m.error = None;
        let req = ParityRequest::ModalSubmit {
            guild,
            channel,
            app: m.spec.app,
            modal: m.spec.id,
            custom_id: m.spec.custom_id.clone(),
            fields,
        };
        self.parity_request(req, cx, |a, res, _| match res {
            Ok(_) => a.parity.modal = None,
            Err(e) => {
                if let Some(m) = a.parity.modal.as_mut() {
                    m.sending = false;
                    m.error = Some(e.0.into());
                }
            }
        });
        cx.notify();
    }
}

fn modal_el(app: &PhoenixApp, cx: &mut gpui::Context<PhoenixApp>) -> Option<AnyElement> {
    let m = app.parity.modal.as_ref()?;
    let close: crate::ui::modal::Action = {
        let e = cx.entity();
        std::rc::Rc::new(move |_, cx| {
            e.update(cx, |a, cx| {
                a.parity.modal = None;
                cx.notify();
            })
        })
    };
    let mut body = div().px(px(16.)).pb(px(16.)).flex().flex_col().gap(px(16.));
    for (spec, f) in m.spec.inputs.iter().zip(&m.inputs) {
        let field = match f {
            Field::Line(i) => Input::new(i).appearance(false).bordered(false).into_any_element(),
            Field::Area(i) => Textarea::new(i).appearance(false).bordered(false).into_any_element(),
        };
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .text_size(px(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(SharedString::from(spec.label.to_uppercase()))
                        .when(spec.required, |d| d.child(div().text_color(c(Palette::RED)).child("*"))),
                )
                .when_some(spec.description.clone(), |d, ds| {
                    d.child(div().text_size(px(14.)).text_color(c(Palette::TEXT_MUTED)).child(ds))
                })
                .child(
                    div()
                        .min_h(px(if spec.style == 2 { 80. } else { 40. }))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(c(Palette::INPUT_BG))
                        .border_1()
                        .border_color(c(Palette::BG_TERTIARY))
                        .text_size(px(16.))
                        .child(field),
                ),
        );
    }
    if let Some(err) = &m.error {
        body = body.child(div().text_size(px(14.)).text_color(c(Palette::TEXT_CRITICAL)).child(err.clone()));
    }
    let e = cx.entity();
    let close2 = close.clone();
    let card = div()
        .w(px(440.))
        .max_h(px(720.))
        .rounded(px(8.))
        .overflow_hidden()
        .bg(c(Palette::BG_PRIMARY))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .when_some(m.spec.app_name.clone(), |d, n| {
                    d.child(div().text_size(px(12.)).text_color(c(Palette::TEXT_MUTED)).child(n))
                })
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(SharedString::from(m.spec.title.clone())),
                ),
        )
        .child(div().id("modal-body").flex_1().min_h_0().overflow_y_scroll().child(body))
        .child(
            div()
                .p(px(16.))
                .bg(c(Palette::BG_SECONDARY))
                .flex()
                .justify_end()
                .items_center()
                .gap(px(16.))
                .child(
                    div()
                        .id("modal-cancel")
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .hover(|s| s.underline())
                        .on_click(move |_, w, cx| close2(w, cx))
                        .child(crate::t!("common.cancel")),
                )
                .child(
                    pill_button("modal-submit", if m.sending { crate::t!("form.submitting") } else { crate::t!("form.submit") }, true, !m.sending)
                        .on_click(move |_, _, cx: &mut App| e.update(cx, |a, cx| a.submit_modal(cx))),
                ),
        );
    Some(crate::ui::modal::frame("interaction-modal", close, card))
}

fn toast(app: &PhoenixApp) -> Option<AnyElement> {
    use crate::theme::{generated::idx, tok, z};
    use gpui::{Animation, AnimationExt as _};
    let t = app.parity.toast.clone()?;
    let card = div()
        .max_w(z(280.))
        .p(z(12.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_NORMAL)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
        .shadow(crate::ui::inbox::shadow_high())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(div().line_clamp(2).child(t.clone())),
        );
    Some(
        gpui::deferred(
            div()
                .absolute()
                .top(z(8.))
                .left_0()
                .w_full()
                .px(z(8.))
                .flex()
                .justify_center()
                .child(card)
                .with_animation(gpui::ElementId::Name(format!("toast-{t}").into()), Animation::new(std::time::Duration::from_millis(300)), |d, p| {
                    d.opacity(p).mt(z(-40. * (1. - p)))
                }),
        )
        .with_priority(30)
        .into_any_element(),
    )
}

pub fn overlays(app: &mut PhoenixApp, window: &mut Window, cx: &mut gpui::Context<PhoenixApp>) -> Vec<AnyElement> {
    if let Some(spec) = app.parity.pending_modal.take() {
        app.parity.modal = Some(build_modal(spec, window, cx));
    }
    crate::ui::forum::prepare(app, window, cx);
    crate::ui::inbox::prepare(app, window, cx);
    let mut out = Vec::new();
    out.extend(crate::ui::inbox::popouts(app, cx));
    out.extend(crate::ui::inbox::dialogs(app, cx));
    out.extend(modal_el(app, cx));
    out.extend(toast(app));
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn text_display_headings_keep_their_size_and_emoji_follow_it() {
        assert_eq!([1, 2, 3].map(super::heading_px), [24., 20., 16.]);
        assert_eq!(super::heading_px(1) / 16. * 22., 33.);
        assert_eq!(super::heading_px(2) / 16. * 22., 27.5);
    }


    #[test]
    fn spoiler_veil_matches_the_client_blur() {
        let a = super::blur_coverage(26.4, 52.85, 44.);
        assert!((0.42..0.47).contains(&a), "{a}");
        assert!(super::blur_coverage(500., 1000., 44.) > 0.99);
        assert!((super::blur_coverage(0., 1000., 44.) - 0.5).abs() < 0.01);
    }

    #[test]
    fn poll_answer_styles_follow_the_client() {
        use crate::theme::generated::idx;
        assert_eq!(super::answer_style(true, true, true), (Some(idx::STATUS_POSITIVE), idx::POLLS_VICTOR_FILL));
        assert_eq!(super::answer_style(true, true, false), (Some(idx::STATUS_POSITIVE), idx::POLLS_VICTOR_FILL));
        assert_eq!(super::answer_style(true, false, true), (None, idx::INTERACTIVE_BACKGROUND_ACTIVE));
        assert_eq!(super::answer_style(false, true, true), (Some(idx::CONTROL_BRAND_FOREGROUND), idx::POLLS_VOTED_FILL));
        assert_eq!(super::answer_style(false, true, false), (None, idx::INTERACTIVE_BACKGROUND_ACTIVE));
    }

    #[test]
    fn button_actions() {
        use phoenix_api::models::{Button, ButtonStyle};
        let link = Button { style: ButtonStyle::Link, url: Some("https://example.com/a?b=1".into()), ..Default::default() };
        assert_eq!(button_action(&link), ButtonAction::Open("https://example.com/a?b=1".into()));
        let go = Button { style: ButtonStyle::Success, custom_id: Some("go".into()), ..Default::default() };
        assert_eq!(button_action(&go), ButtonAction::Interact("go".into()));
        assert_eq!(button_action(&Button { style: ButtonStyle::Premium, ..Default::default() }), ButtonAction::Store);
        assert_eq!(button_action(&Button { style: ButtonStyle::Link, ..Default::default() }), ButtonAction::None);
        assert_eq!(button_tokens(ButtonStyle::Premium), button_tokens(ButtonStyle::Primary));
        assert_eq!(button_tokens(ButtonStyle::Link), button_tokens(ButtonStyle::Secondary));
        assert_eq!(button_tokens(ButtonStyle::Danger)[0], idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT);
    }

    #[test]
    fn media_items_become_attachments() {
        use phoenix_api::models::UnfurledMedia;
        let m = UnfurledMedia {
            url: "https://cdn.example/a/b.png?ex=1".into(),
            proxy_url: Some("https://media.example/a/b.png?ex=1".into()),
            width: Some(800),
            height: Some(400),
            content_type: Some("image/png".into()),
            ..Default::default()
        };
        let a = as_attachment(&m, None, 0, true);
        assert_eq!((a.filename.as_ref(), a.url.as_ref(), a.width, a.spoiler), ("b.png", "https://media.example/a/b.png?ex=1", 800, true));
        assert_eq!(a.kind, crate::model::AttachmentKind::Image);
        let f = UnfurledMedia { url: "https://cdn.example/t.html".into(), width: Some(0), height: Some(0), content_type: Some("text/html".into()), ..Default::default() };
        let a = as_attachment(&f, Some("t.html"), 40104, false);
        assert_eq!((a.kind, a.url.as_ref(), a.size), (crate::model::AttachmentKind::File, "https://cdn.example/t.html", 40104));
    }

    use super::*;
    use crate::commands::store::PollAnswer;

    #[test]
    fn percents() {
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 67);
        assert_eq!(percent(0, 0), 0);
    }

    #[test]
    fn own_vote_replaces_previous() {
        let a = |id, count, me| PollAnswer { id, text: String::new(), emoji: None, count, me };
        let mut p = PollView { question: "q".into(), answers: vec![a(1, 2, true), a(2, 0, false)], multi: false, expiry: None, finalized: false };
        apply_own_vote(&mut p, &[2]);
        assert_eq!((p.answers[0].count, p.answers[0].me), (1, false));
        assert_eq!((p.answers[1].count, p.answers[1].me), (1, true));
        apply_own_vote(&mut p, &[]);
        assert_eq!(p.total(), 1);
        assert!(!p.voted());
    }

    #[test]
    fn hours_left() {
        assert_eq!(time_left(7_200), "2h left");
        assert_eq!(time_left(90_000), "1d left");
    }
}
