use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");
pub const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../../packaging/icons/discord-phoenix.png");

macro_rules! s {
    ($body:expr) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            $body,
            "</svg>"
        )
    };
}
macro_rules! f {
    ($body:expr) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            $body,
            "</svg>"
        )
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Hash,
    Speaker,
    Megaphone,
    Forum,
    Stage,
    Chevron,
    Plus,
    Mic,
    MicOff,
    Headphones,
    HeadphonesOff,
    Gear,
    Bell,
    Pin,
    Members,
    Search,
    Inbox,
    Help,
    Gift,
    Gif,
    Sticker,
    Emoji,
    PlusCircle,
    Compass,
    Download,
    Close,
    Threads,
    Friends,
    Nitro,
    Lock,
    Reply,
    File,
    Image,
    PhoneHangup,
    Quests,
    Shop,
    Envelope,
    Clyde,
    Send,
}

impl Icon {
    pub fn path(self) -> SharedString {
        crate::icons::path(match self {
            Icon::Hash => "hash",
            Icon::Speaker => "speaker",
            Icon::Megaphone => "megaphone",
            Icon::Forum => "forum",
            Icon::Stage => "stage",
            Icon::Chevron => "chevron",
            Icon::Plus => "plus",
            Icon::Mic => "mic",
            Icon::MicOff => "mic-off",
            Icon::Headphones => "headphones",
            Icon::HeadphonesOff => "headphones-off",
            Icon::Gear => "gear",
            Icon::Bell => "bell",
            Icon::Pin => "pin",
            Icon::Members => "members",
            Icon::Search => "search",
            Icon::Inbox => "inbox",
            Icon::Help => "help",
            Icon::Gift => "gift",
            Icon::Gif => "gif",
            Icon::Sticker => "sticker",
            Icon::Emoji => "emoji",
            Icon::PlusCircle => "plus-circle",
            Icon::Compass => "compass",
            Icon::Download => "download",
            Icon::Close => "close",
            Icon::Threads => "threads",
            Icon::Friends => "friends",
            Icon::Nitro => "nitro",
            Icon::Lock => "lock",
            Icon::Reply => "reply",
            Icon::File => "file",
            Icon::Image => "image",
            Icon::PhoneHangup => "phone-hangup",
            Icon::Quests => "quests",
            Icon::Shop => "shop",
            Icon::Envelope => "server/envelope",
            Icon::Clyde => "discord-logo",
            Icon::Send => "send",
        })
    }
}

pub fn fallback_svg(path: &str) -> Option<&'static str> {
    svg_for(path)
}

fn svg_for(path: &str) -> Option<&'static str> {
    Some(match path {
        "icons/double-chevron-right.svg" => s!(r#"<path d="m6 6 6 6-6 6M12 6l6 6-6 6"/>"#),
        "icons/paintbrush-thin.svg" => s!(r#"<path d="m14 4 6 6-9 9a3 3 0 0 1-4.2 0L5 17.2a3 3 0 0 1 0-4.2zM3 21l3-3"/>"#),
        "icons/hash.svg" | "icons/text-nsfw.svg" => s!(r#"<path d="M10 3 8 21M16 3l-2 18M4 8.5h17M3 15.5h17"/>"#),
        "icons/speaker.svg" | "icons/voice-nsfw.svg" => f!(r#"<path d="M11.4 3.2A1 1 0 0 1 13 4v16a1 1 0 0 1-1.6.8L6.7 17H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h3.7zM15.5 8.5a1 1 0 0 1 1.4 0 5 5 0 0 1 0 7 1 1 0 1 1-1.4-1.4 3 3 0 0 0 0-4.2 1 1 0 0 1 0-1.4zm2.8-2.8a1 1 0 0 1 1.4 0 9 9 0 0 1 0 12.6 1 1 0 1 1-1.4-1.4 7 7 0 0 0 0-9.8 1 1 0 0 1 0-1.4z"/>"#),
        "icons/megaphone.svg" | "icons/announcement-nsfw.svg" => s!(r#"<path d="M3 10v4a1 1 0 0 0 1 1h3l8 5V4L7 9H4a1 1 0 0 0-1 1zM7 15l1.5 5M19 9.5a3 3 0 0 1 0 5"/>"#),
        "icons/forum.svg" | "icons/forum-nsfw.svg" => s!(r#"<path d="M4 4h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H8l-4 3V5a1 1 0 0 1 1-1zM20 8h0a1 1 0 0 1 1 1v11l-3-2.5H10a1 1 0 0 1-1-1V17"/>"#),
        "icons/stage.svg" => s!(r#"<circle cx="12" cy="9" r="3"/><path d="M5.6 15.5A8 8 0 1 1 18.4 15.5M8 21l1-5h6l1 5"/>"#),
        "icons/sparkle.svg" => f!(r#"<path d="M12 2l2.2 6.6L21 11l-6.8 2.4L12 20l-2.2-6.6L3 11l6.8-2.4z"/>"#),
        "icons/picker/sparkle.svg" => f!(r#"<path d="M12 2l2.2 6.6L21 11l-6.8 2.4L12 20l-2.2-6.6L3 11l6.8-2.4z"/>"#),
        "icons/event/location.svg" => s!(r#"<path d="M12 21s-6.5-6.2-6.5-11a6.5 6.5 0 0 1 13 0c0 4.8-6.5 11-6.5 11z"/><circle cx="12" cy="10" r="2.5"/>"#),
        "icons/chevron.svg" => s!(r#"<path d="m6 9 6 6 6-6"/>"#),
        "icons/friends/discovery-arrow.svg" => s!(r#"<path d="m9 6 6 6-6 6"/>"#),
        "icons/x-neutral.svg" => f!(r#"<path d="M13.9 10.6 21 2.5h-1.7l-6.2 7L8.2 2.5H2.5l7.5 10.6-7.5 8.4h1.7l6.5-7.4 5.2 7.4h5.7zm-2.3 2.6-.8-1.1L4.8 3.8h2.6l4.9 6.9.8 1.1 6.3 8.9h-2.6z"/>"#),
        "icons/boot/status.svg" => s!(r#"<circle cx="12" cy="12" r="4"/><path d="M4 12a8 8 0 0 1 13.7-5.7M20 12a8 8 0 0 1-13.7 5.7"/>"#),
        "icons/connection-fine.svg" => s!(r#"<path d="M4 4a16 16 0 0 1 16 16M4 9a11 11 0 0 1 11 11M4 14a6 6 0 0 1 6 6M4 20h.01"/>"#),
        "icons/connection-average.svg" => s!(r#"<path d="M4 9a11 11 0 0 1 11 11M4 14a6 6 0 0 1 6 6M4 20h.01"/>"#),
        "icons/connection-bad.svg" => s!(r#"<path d="M4 14a6 6 0 0 1 6 6M4 20h.01"/>"#),
        "icons/connection-unknown.svg" => s!(r#"<path d="M4 14a6 6 0 0 1 6 6M4 20h.01M18 14v3M18 20h.01"/>"#),
        "icons/shield.svg" => s!(r#"<path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.4 7.5 9.5 4.3-1.1 7.5-4.9 7.5-9.5V6z"/>"#),
        "icons/settings-info.svg" => s!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>"#),
        "icons/media/window-top.svg" => s!(r#"<path d="M4 5h16v14H4zM4 9h16"/>"#),
        "icons/media/retry.svg" => s!(r#"<path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v4h-4"/>"#),
        "icons/media/speedometer.svg" => s!(r#"<path d="M4.9 18A9 9 0 1 1 19.1 18M12 13l4-5"/>"#),
        "icons/media/replay.svg" => f!(r#"<path d="M12 5V1L7 6l5 5V7a6 6 0 1 1-6 6H4a8 8 0 1 0 8-8z"/>"#),
        "icons/settings/cloud.svg" => f!(r#"<path d="M7 19a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9a5 5 0 0 1-.5 10z"/>"#),
        "icons/invite.svg" => s!(r#"<circle cx="10" cy="8" r="4"/><path d="M3 20a7 7 0 0 1 12-4.9M19 14v6m-3-3h6"/>"#),
        "icons/nav/back.svg" => s!(r#"<path d="M20 12H5m6-7-7 7 7 7"/>"#),
        "icons/title/back.svg" => s!(r#"<path d="M19 12H6m5-6-6 6 6 6"/>"#),
        "icons/title/forward.svg" => s!(r#"<path d="M5 12h13m-5-6 6 6-6 6"/>"#),
        "icons/arrow-left.svg" => concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#, r#"<polygon points="20 11 20 13 8 13 13.5 18.5 12.08 19.92 4.16 12 12.08 4.08 13.5 5.5 8 11"/>"#, "</svg>"),
        "icons/chat/unread-cap.svg" => concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 13" fill="black">"#, r#"<path d="M8 0v13H5L0 6.5 5 0z"/>"#, "</svg>"),
        "icons/chat-check.svg" => s!(r#"<path d="M21 12a9 9 0 0 1-13.5 7.8L3 21l1.2-4.5A9 9 0 1 1 21 12z"/><path d="m8.5 12 2.5 2.5 4.5-5"/>"#),
        "icons/chat/eye-slash.svg" => s!(r#"<path d="M3 3l18 18M10.6 6.1A9.8 9.8 0 0 1 12 6c5 0 8.5 4 9.5 6a12.6 12.6 0 0 1-2.9 3.6M6.3 7.6C4.4 8.8 3.1 10.5 2.5 12c1 2 4.5 6 9.5 6 1.5 0 2.8-.3 4-.9M9.9 9.9a3 3 0 0 0 4.2 4.2"/>"#),
        "icons/chat/sticker-small.svg" => s!(r#"<path d="M14 3H6a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3h6l9-9V6a3 3 0 0 0-3-3zM12 21v-4a5 5 0 0 1 5-5h4"/>"#),
        "icons/chat/slash-box.svg" => s!(r#"<rect x="3" y="3" width="18" height="18" rx="3"/><path d="m14.5 7.5-5 9"/>"#),
        "icons/settings/nitro-wordmark.svg" => concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 106 26" fill="black">"#, r#"<path d="M2 25 6 1h4l5 14 2-14h5l-4 24h-4L9 11 7 25z"/>"#, "</svg>"),
        "icons/chat/forwarded.svg" => s!(r#"<path d="M20 8h-7a8 8 0 0 0-8 8v4M15 3l5 5-5 5"/>"#),
        "icons/menu/folder-plus.svg" => s!(r#"<path d="M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1zM12 10v6m-3-3h6"/>"#),
        "icons/menu/view-reactions.svg" => s!(r#"<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="3"/>"#),
        "icons/menu/remove-reactions.svg" => s!(r#"<circle cx="12" cy="12" r="9" stroke-dasharray="3 3"/><path d="M8 12h8"/>"#),
        "icons/menu/remove-all-reactions.svg" => s!(r#"<circle cx="11" cy="11" r="8"/><path d="M15 19h6"/>"#),
        "icons/menu/open-link.svg" => s!(r#"<path d="M9 6h9v9M18 6 6 18"/>"#),
        "icons/link-external.svg" => s!(r#"<path d="M9 6h9v9M18 6 6 18"/>"#),
        "icons/viewer/zoom.svg" => s!(r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5M10.5 7.5v6m-3-3h6"/>"#),
        "icons/heart.svg" => s!(r#"<path d="M12 20s-7.5-4.6-9-9.3C2 7.4 4.3 4.5 7.4 4.5c2 0 3.6 1.2 4.6 2.8 1-1.6 2.6-2.8 4.6-2.8 3.1 0 5.4 2.9 4.4 6.2C19.5 15.4 12 20 12 20z"/>"#),
        "icons/heart-outline.svg" => s!(r#"<path d="M12 20s-7.5-4.6-9-9.3C2 7.4 4.3 4.5 7.4 4.5c2 0 3.6 1.2 4.6 2.8 1-1.6 2.6-2.8 4.6-2.8 3.1 0 5.4 2.9 4.4 6.2C19.5 15.4 12 20 12 20z"/>"#),
        "icons/server/badge.svg" => s!(r#"<path d="M6 3h12l-1.5 15L12 21l-4.5-3z"/><circle cx="12" cy="10" r="2.5"/>"#),
        "icons/server/envelope.svg" => s!(r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>"#),
        "icons/server/clipboard.svg" => s!(r#"<rect x="5" y="4" width="14" height="17" rx="2"/><path d="M9 3h6v3H9zM8 11h8M8 15h8"/>"#),
        "icons/server/server.svg" => s!(r#"<rect x="3" y="3" width="18" height="18" rx="5"/><path d="M8 12h8"/>"#),
        "icons/server/list-numbered.svg" => s!(r#"<path d="M10 6h11M10 12h11M10 18h11M4 5l1-1v4M3.5 11h2l-2 3h2M3.5 17h2v3h-2"/>"#),
        "icons/server/grid-horizontal.svg" => s!(r#"<rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/>"#),
        "icons/server/server-grid.svg" => s!(r#"<rect x="3" y="3" width="8" height="8" rx="2"/><rect x="13" y="3" width="8" height="8" rx="2"/><rect x="3" y="13" width="8" height="8" rx="2"/><rect x="13" y="13" width="8" height="8" rx="2"/>"#),
        "icons/access/list-bullets.svg" => f!(r#"<circle cx="4" cy="6" r="1.5"/><circle cx="4" cy="12" r="1.5"/><circle cx="4" cy="18" r="1.5"/><rect x="8" y="5" width="13" height="2" rx="1"/><rect x="8" y="11" width="13" height="2" rx="1"/><rect x="8" y="17" width="13" height="2" rx="1"/>"#),
        "icons/access/channel-list.svg" => f!(r#"<rect x="3" y="5" width="18" height="2" rx="1"/><rect x="3" y="11" width="18" height="2" rx="1"/><rect x="3" y="17" width="12" height="2" rx="1"/>"#),
        "icons/access/plus-small.svg" => s!(r#"<path d="M12 7v10M7 12h10"/>"#),
        "icons/server/drag.svg" => f!(r#"<circle cx="8" cy="4" r="2"/><circle cx="8" cy="12" r="2"/><circle cx="8" cy="20" r="2"/><circle cx="16" cy="4" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="16" cy="20" r="2"/>"#),
        "icons/server/dropper.svg" => s!(r#"<path d="m14 7 3 3-9 9H5v-3zM14 4l6 6-2 2-6-6z"/>"#),
        "icons/tag.svg" => s!(r#"<path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9z"/><circle cx="8" cy="8" r="1.5"/>"#),
        "icons/viewer/open.svg" => s!(r#"<path d="M13 4h7v7M20 4l-9 9"/><path d="M18 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h5"/>"#),
        "icons/circle-check.svg" => f!(r#"<path fill-rule="evenodd" d="M12 1a11 11 0 1 0 0 22 11 11 0 0 0 0-22zm-1.3 15.4-4.4-4.4 1.4-1.4 3 3 6.3-6.3 1.4 1.4z"/>"#),
        "icons/circle-check-bg.svg" => f!(r#"<circle cx="12" cy="12" r="10"/>"#),
        "icons/attach/poll.svg" => s!(r#"<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M7 9h6M7 13h10M7 17h4"/>"#),
        "icons/chat/timer.svg" => s!(r#"<circle cx="12" cy="13" r="8"/><path d="M10 2h4M12 2v3M12 13l3-3M18.5 6.5l1-1"/>"#),
        "icons/send.svg" => s!(r#"<path d="m22 2-9 20-4-9-7-4z"/><path d="M22 2 9 13"/>"#),
        "icons/menu/bookmark.svg" => s!(r#"<path d="M7 4h10v16l-5-4-5 4z"/>"#),
        "icons/menu/speak.svg" => s!(r#"<path d="M21 12a9 9 0 1 0-16.5 5L3 21h9a9 9 0 0 0 9-9z"/><path d="M14 9a3 3 0 0 1 0 6"/>"#),
        "icons/nav/forward.svg" => s!(r#"<path d="M4 12h15m-6-7 7 7-7 7"/>"#),
        "icons/nav/up.svg" => s!(r#"<path d="M12 20V5m-7 6 7-7 7 7"/>"#),
        "icons/forum/tag.svg" => s!(r#"<path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9z"/><circle cx="8" cy="8" r="1.5"/>"#),
        "icons/calendar.svg" => s!(r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4m8-4v4"/>"#),
        "icons/signpost.svg" => s!(r#"<path d="M12 3v18M8 21h8M5 6h12l3 3-3 3H5z"/>"#),
        "icons/channel-browse.svg" => s!(r#"<path d="M4 6h12M4 12h7M4 18h5"/><circle cx="17" cy="16" r="3"/><path d="m19.5 18.5 2 2"/>"#),
        "icons/boost.svg" => s!(r#"<path d="M12 2 4 6v6c0 5 3.5 8.5 8 10 4.5-1.5 8-5 8-10V6z"/><path d="m12 7-3 5h6l-3 5"/>"#),
        "icons/chevron-right.svg" => s!(r#"<path d="m9 6 6 6-6 6"/>"#),
        "icons/chevron-small-left.svg" => s!(r#"<path d="m15 6-6 6 6 6"/>"#),
        "icons/mod/chevron-large-right.svg" => s!(r#"<path d="m8 4 8 8-8 8"/>"#),
        "icons/mod/user-minus.svg" => s!(r#"<circle cx="10" cy="8" r="4"/><path d="M3 20a7 7 0 0 1 12-4.9M16 17h6"/>"#),
        "icons/mod/hammer.svg" => s!(r#"<path d="m14 4 6 6-3 3-6-6zM12.5 8.5 4 17l3 3 8.5-8.5"/>"#),
        "icons/mod/clock-warning.svg" => s!(r#"<path d="M20.5 11A8.5 8.5 0 1 0 12 20.5M12 7v5l3 2M19 15v3m0 3h0"/>"#),
        "icons/mod/chat-warning.svg" => s!(r#"<path d="M21 12a9 9 0 0 1-13.5 7.8L3 21l1.2-4.5A9 9 0 1 1 21 12zM12 7.5v5m0 3.5h0"/>"#),
        "icons/mod/robot.svg" => s!(r#"<rect x="4" y="8" width="16" height="12" rx="3"/><path d="M12 4v4M9 13h0m6 0h0"/>"#),
        "icons/user.svg" => s!(r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>"#),
        "icons/shield-lock.svg" => s!(r#"<path d="M12 2 4 6v6c0 5 3.5 8.5 8 10 4.5-1.5 8-5 8-10V6z"/><rect x="9" y="11" width="6" height="5" rx="1"/><path d="M10 11V9a2 2 0 0 1 4 0v2"/>"#),
        "icons/chat-shield.svg" => s!(r#"<path d="M20 11a8 8 0 1 0-15 4l-1 5 5-1a8 8 0 0 0 3 .6"/><path d="M18 13l-3 1.5v2c0 1.8 1.3 3 3 3.5 1.7-.5 3-1.7 3-3.5v-2z"/>"#),
        "icons/credit-card.svg" => s!(r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20M6 15h4"/>"#),
        "icons/qr-code.svg" => s!(r#"<path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3"/><rect x="7" y="7" width="4" height="4"/><rect x="13" y="13" width="4" height="4"/>"#),
        "icons/piggy-bank.svg" => s!(r#"<path d="M4 11a7 6 0 0 1 13-3h2v3l2 1v3l-2 1-2 3h-2l-1-2h-4l-1 2H7l-1.5-3A6 6 0 0 1 4 11z"/><circle cx="15" cy="11" r=".5"/>"#),
        "icons/phone.svg" => s!(r#"<path d="M7 3H5a2 2 0 0 0-2 2c0 9 7 16 16 16a2 2 0 0 0 2-2v-2l-4.5-1.5-2 2a12 12 0 0 1-6-6l2-2z"/>"#),
        "icons/subscription.svg" => s!(r#"<path d="M20 12a8 8 0 0 1-14 5.3M4 12a8 8 0 0 1 14-5.3M18 3v4h-4M6 21v-4h4"/><path d="M14 9.5c-.5-.8-1.2-1-2-1-1.2 0-2 .6-2 1.5 0 2 4 1 4 3 0 .9-.8 1.5-2 1.5-.8 0-1.5-.3-2-1M12 7.5v9"/>"#),
        "icons/accessibility.svg" => s!(r#"<circle cx="12" cy="4.5" r="1.5"/><path d="M5 8l7 1.5L19 8M12 9.5V14l-3 7m3-7 3 7"/>"#),
        "icons/keyboard.svg" => s!(r#"<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h0m4 0h0m4 0h0m4 0h0M7 14h10"/>"#),
        "icons/language.svg" => s!(r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3.5 3 14.5 0 18M12 3c-3 3.5-3 14.5 0 18"/>"#),
        "icons/user-platform.svg" => s!(r#"<circle cx="9" cy="8" r="3.5"/><path d="M2 20a7 7 0 0 1 12-4.9"/><rect x="15" y="13" width="7" height="6" rx="1"/>"#),
        "icons/link.svg" => s!(r#"<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>"#),
        "icons/angle-brackets.svg" => s!(r#"<path d="m8 6-6 6 6 6M16 6l6 6-6 6"/>"#),
        "icons/folder.svg" => f!(r#"<path d="M3 6a2 2 0 0 1 2-2h4.5l2 2H19a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#),
        "icons/door-exit.svg" => s!(r#"<path d="M14 4H5v16h9M10 12h11m-3-3 3 3-3 3"/>"#),
        "icons/discord-logo.svg" => f!(r#"<path d="M19.7 5.4a17 17 0 0 0-4.3-1.3l-.5 1.1a15.7 15.7 0 0 0-4.8 0l-.5-1.1a17 17 0 0 0-4.3 1.3C2.6 9.5 1.9 13.5 2.2 17.4a17.2 17.2 0 0 0 5.3 2.7l1.1-1.8a11 11 0 0 1-1.7-.8l.4-.3a12.3 12.3 0 0 0 10.4 0l.4.3a11 11 0 0 1-1.7.8l1.1 1.8a17.1 17.1 0 0 0 5.3-2.7c.4-4.5-.8-8.5-3.1-12zM8.7 15c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1zm6.6 0c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1z"/>"#),
        "icons/music.svg" => f!(r#"<path d="M20 3v12.5a3.5 3.5 0 1 1-2-3.2V7.3l-8 1.6v8.6A3.5 3.5 0 1 1 8 14.3V5.4z"/>"#),
        "icons/tv.svg" => s!(r#"<rect x="3" y="5" width="18" height="12" rx="2"/><path d="M8 21h8M12 17v4"/>"#),
        "icons/game-controller.svg" => f!(r#"<path d="M7 7h10a5 5 0 0 1 5 5v1a4 4 0 0 1-7 2.6L14 14h-4l-1 1.6A4 4 0 0 1 2 13v-1a5 5 0 0 1 5-5zm0 3v2H5v2h2v2h2v-2h2v-2H9v-2zm9 1a1 1 0 1 0 0 2 1 1 0 0 0 0-2z"/>"#),
        "icons/more-horizontal.svg" => f!(r#"<path d="M4 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm8 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm8 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"/>"#),
        "icons/medal.svg" => f!(r#"<path d="M7 2h4l1 4 1-4h4l-3 7a6 6 0 1 1-4 0zM12 11a4 4 0 1 0 0 8 4 4 0 0 0 0-8z"/>"#),
        "icons/streak.svg" => f!(r#"<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>"#),
        "icons/fire.svg" => f!(r#"<path d="M12 2c1 3.5 5 6 5 11a5 5 0 0 1-10 0c0-2.3 1.2-3.9 2.5-5 .2 1.8 1 3 2.5 3.5-.8-3.2.2-6.3 0-9.5z"/>"#),
        "icons/shop.svg" => f!(r#"<path d="M4 4h16l-1.5 6H5.5zM5 11h14v9H5zM9 14h6v6H9z"/>"#),
        "icons/shop-sparkle.svg" => f!(r#"<path d="M4 4h12l-1.5 6H5.5zM5 11h11v9H5zM19 2l.9 2.1L22 5l-2.1.9L19 8l-.9-2.1L16 5l2.1-.9z"/>"#),
        "icons/flag.svg" => f!(r#"<path d="M5 3h1.5v18H5zM7.5 4h10.2l-2.2 4 2.2 4H7.5z"/>"#),
        "icons/star.svg" => f!(r#"<path d="M12 2.5l2.9 6 6.6.8-4.9 4.5 1.3 6.5L12 17l-5.9 3.3 1.3-6.5-4.9-4.5 6.6-.8z"/>"#),
        "icons/star-outline.svg" => s!(r#"<path d="M12 2.5l2.9 6 6.6.8-4.9 4.5 1.3 6.5L12 17l-5.9 3.3 1.3-6.5-4.9-4.5 6.6-.8z"/>"#),
        "icons/discord.svg" => f!(r#"<path d="M19.3 5.3A17 17 0 0 0 15 4l-.5 1a15.7 15.7 0 0 0-5 0L9 4a17 17 0 0 0-4.3 1.3C2 9.4 1.3 13.4 1.6 17.3A17.2 17.2 0 0 0 6.9 20l1.1-1.8a11 11 0 0 1-1.7-.8l.4-.3a12.3 12.3 0 0 0 10.6 0l.4.3-1.7.8 1.1 1.8a17.1 17.1 0 0 0 5.3-2.7c.4-4.5-.7-8.5-3.1-12zM8.7 14.9c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1zm6.6 0c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1z"/>"#),
        "icons/new-user.svg" => f!(r#"<path d="M12 11a4 4 0 0 1-4-4 4 4 0 0 1 8 0 4 4 0 0 1-4 4zm0 0a4 4 0 0 1 4 4 4 4 0 0 1-8 0 4 4 0 0 1 4-4zm0 0a4 4 0 0 1-4 4 4 4 0 0 1 0-8 4 4 0 0 1 4 4zm0 0a4 4 0 0 1 4-4 4 4 0 0 1 0 8 4 4 0 0 1-4-4zM12 18v4"/>"#),
        "icons/crown.svg" => f!(r#"<path d="M3 7l4.5 4L12 4l4.5 7L21 7l-2 12H5z"/>"#),
        "icons/sort.svg" => s!(r#"<path d="M7 20V4M3 8l4-4 4 4M17 4v16M13 16l4 4 4-4"/>"#),
        "icons/chevron-up.svg" => s!(r#"<path d="m6 15 6-6 6 6"/>"#),
        "icons/check.svg" => s!(r#"<path d="m5 12.5 4.5 4.5L19 7.5"/>"#),
        "icons/check-small.svg" => s!(r#"<path d="m7.5 12.5 3 3L16.5 9"/>"#),
        "icons/guild/flower-star.svg" => f!(r#"<path d="M12 1.5l2.6 2.2 3.4-.4 1 3.3 3 1.6-1 3.3 1 3.3-3 1.6-1 3.3-3.4-.4-2.6 2.2-2.6-2.2-3.4.4-1-3.3-3-1.6 1-3.3-1-3.3 3-1.6 1-3.3 3.4.4z"/>"#),
        "icons/guild/home.svg" => f!(r#"<path d="M3 10.5 12 3l9 7.5V21h-6v-6H9v6H3z"/>"#),
        "icons/guild/partner.svg" => f!(r#"<path d="M15.9 9.6 13.8 11c-.3.1-.5.1-.8 0-.6-.3-1.3-.2-1.8.2l-4.7 3.1c-.9.5-2 .3-2.4-.6-.5-.7-.2-1.8.5-2.4L9 8.3c1.2-.8 2.7-1.1 4-.8 1.2.3 2.1.9 2.9 1.8.2.1.1.2 0 .3zM20.2 11.7c0 .6-.3 1.2-.7 1.5L14.8 16.2c-.9.6-1.8.9-2.8.9-.9 0-2.6-.5-3.6-1.8-.1-.2 0-.5.1-.6l2.1-1.4c.3-.1.6 0 .8.2.6.3 1.3.2 1.8-.2l4.7-3.1c.8-.5 2-.3 2.4.6.1.3.1.6 0 .9z"/>"#),
        "icons/guild/staff.svg" => f!(r#"<path d="M2 20.6v-1.2l.3-.7 2.4-2.4.7-.3H6l9-9-.9-.9-1.7-4 .7-.6 4 1.7.6.6 1.4 0 1 1v1.4l1.6 1.6v1.4l-1.6 1.6h-1.4L17 9l-9 9v.6l-.3.7-2.4 2.4-.7.3H3.4l-.7-.3-.4-.4z"/>"#),
        "icons/check-small-bold.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="m6 12 4 4 8-8"/>"#,
            "</svg>"
        ),
        "icons/plus.svg" => s!(r#"<path d="M12 5v14M5 12h14"/>"#),
        "icons/plus-small.svg" => s!(r#"<path d="M12 7v10M7 12h10"/>"#),
        "icons/sys/ticket.svg" => s!(r#"<path d="M3 7a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v2.5a2.5 2.5 0 0 0 0 5V17a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-2.5a2.5 2.5 0 0 0 0-5zM15 5v14"/>"#),
        "icons/sys/hand-request-speak.svg" => s!(r#"<path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V11m0-6.5a1.5 1.5 0 0 1 3 0V11m0-5a1.5 1.5 0 0 1 3 0v5m0-2.5a1.5 1.5 0 0 1 3 0V15a7 7 0 0 1-12.6 4.2L5 15.5a1.5 1.5 0 0 1 2.4-1.8L8 14.5"/>"#),
        "icons/stage/group-arrow-down.svg" => s!(r#"<circle cx="9" cy="7" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0M19 4v9m-3-3 3 3 3-3"/>"#),
        "icons/sys/mic-arrow-right.svg" => s!(r#"<rect x="6" y="2" width="7" height="12" rx="3.5"/><path d="M3 11a6.5 6.5 0 0 0 13 0M9.5 17.5V21M15 18h7m-3-3 3 3-3 3"/>"#),
        "icons/mic.svg" => f!(r#"<rect x="8" y="2" width="8" height="13" rx="4"/><path d="M5 11a1 1 0 0 1 1 1 6 6 0 0 0 12 0 1 1 0 1 1 2 0 8 8 0 0 1-7 7.9V22h3a1 1 0 1 1 0 2H8a1 1 0 1 1 0-2h3v-2.1A8 8 0 0 1 4 12a1 1 0 0 1 1-1z"/>"#),
        "icons/mic-off.svg" | "icons/mic-deny.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">"#,
            r#"<rect x="8" y="2" width="8" height="13" rx="4" fill="black"/><path d="M5 11a1 1 0 0 1 1 1 6 6 0 0 0 12 0 1 1 0 1 1 2 0 8 8 0 0 1-7 7.9V22h3a1 1 0 1 1 0 2H8a1 1 0 1 1 0-2h3v-2.1A8 8 0 0 1 4 12a1 1 0 0 1 1-1z" fill="black"/>"#,
            r#"<path d="M3 2 21 22" stroke="black" stroke-width="2.4" stroke-linecap="round"/></svg>"#
        ),
        "icons/headphones.svg" => f!(r#"<path d="M12 3a9 9 0 0 0-9 9v7a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5v-1a7 7 0 0 1 14 0v1h-2a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-7a9 9 0 0 0-9-9z"/>"#),
        "icons/headphones-off.svg" | "icons/headphones-deny.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">"#,
            r#"<path fill="black" d="M12 3a9 9 0 0 0-9 9v7a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5v-1a7 7 0 0 1 14 0v1h-2a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-7a9 9 0 0 0-9-9z"/>"#,
            r#"<path d="M3 2 21 22" stroke="black" stroke-width="2.4" stroke-linecap="round"/></svg>"#
        ),
        "icons/gear.svg" => f!(r#"<path fill-rule="evenodd" d="M10.6 2h2.8l.5 2.6 1.7.7 2.2-1.5 2 2-1.5 2.2.7 1.7 2.6.5v2.8l-2.6.5-.7 1.7 1.5 2.2-2 2-2.2-1.5-1.7.7-.5 2.6h-2.8l-.5-2.6-1.7-.7-2.2 1.5-2-2 1.5-2.2-.7-1.7L2 13.4v-2.8l2.6-.5.7-1.7-1.5-2.2 2-2 2.2 1.5 1.7-.7zM12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z"/>"#),
        "icons/bell.svg" => f!(r#"<path d="M12 2a7 7 0 0 0-7 7v4l-2 3v1h18v-1l-2-3V9a7 7 0 0 0-7-7zM9 19a3 3 0 0 0 6 0z"/>"#),
        "icons/pin.svg" => f!(r#"<path d="M15 2 22 9l-2 1-3.5 3.5L17 19l-1.5 1.5-4.5-4.5-6 6L3 20l6-6-4.5-4.5L6 8l5.5.5L15 5z"/>"#),
        "icons/members.svg" => f!(r#"<circle cx="9" cy="8" r="4"/><path d="M1 20a8 6 0 0 1 16 0v1H1zM16 4a4 4 0 0 1 0 8 6 6 0 0 0 0-8zM18 14a7 6 0 0 1 5 6v1h-4v-1a8 8 0 0 0-1-6z"/>"#),
        "icons/search.svg" => s!(r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m20 20-4.8-4.8"/>"#),
        "icons/inbox.svg" => f!(r#"<path d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm0 2v9h3.5a3.5 3.5 0 0 0 7 0H19V5z"/>"#),
        "icons/help.svg" => f!(r#"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm0 14.5a1.3 1.3 0 1 0 0 2.6 1.3 1.3 0 0 0 0-2.6zM12 5a4 4 0 0 0-4 4h2.2a1.8 1.8 0 1 1 2.6 1.6c-1.2.6-1.9 1.4-1.9 2.9v.5h2.2v-.4c0-.6.3-1 1-1.4A4 4 0 0 0 12 5z"/>"#),
        "icons/gift.svg" => f!(r#"<path d="M3 8h18v4H3zM4 13h7v8H5a1 1 0 0 1-1-1zm9 0h7v7a1 1 0 0 1-1 1h-6zM11 8V7a3 3 0 1 0-3 0zM13 8V7a3 3 0 1 1 3 0z"/>"#),
        "icons/gif.svg" => f!(r#"<path fill-rule="evenodd" d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm4.5 5A2.5 2.5 0 0 0 6 11.5v1A2.5 2.5 0 0 0 8.5 15h1.5v-3H8.5v1.2H8v-1.7a.5.5 0 0 1 .5-.5H10V9zm3 0v6h1.6V9zm2.8 0v6h1.6v-2H18v-1.5h-2.1V10.5H18.2V9z"/>"#),
        "icons/sticker.svg" => f!(r#"<path d="M5 3h14a2 2 0 0 1 2 2v9l-7 7H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm8 18v-5a2 2 0 0 1 2-2h6z"/>"#),
        "icons/emoji.svg" => f!(r#"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zM8.5 8a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm7 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zM6.5 13.5a5.5 5.5 0 0 0 11 0z"/>"#),
        "icons/plus-circle.svg" => f!(r#"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm-1 5v4H7v2h4v4h2v-4h4v-2h-4V7z"/>"#),
        "icons/compass.svg" => f!(r#"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm4.5 5.5-6.4 2.6-2.6 6.4 6.4-2.6zM12 10.7a1.3 1.3 0 1 1 0 2.6 1.3 1.3 0 0 1 0-2.6z"/>"#),
        "icons/download.svg" => s!(r#"<path d="M12 3v12m-5-5 5 5 5-5M4 21h16"/>"#),
        "icons/window/close.svg" => concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12">"#, r#"<path d="M1.3 1.3 10.7 10.7M10.7 1.3 1.3 10.7" stroke="black" stroke-width="0.82"/>"#, "</svg>"),
        "icons/close.svg" => s!(r#"<path d="M6 6l12 12M18 6 6 18"/>"#),
        "icons/threads.svg" => s!(r#"<path d="M5 4h14v10H11l-4 3v-3H5zM9 8h6M9 11h4"/>"#),
        "icons/friends.svg" => f!(r#"<circle cx="12" cy="7" r="4.5"/><path d="M3 21a9 7 0 0 1 18 0z"/>"#),
        "icons/nitro.svg" => f!(r#"<path d="M3 9h5v2H3zm1 4h4v2H4zM14 4a8 8 0 1 1 0 16 8 8 0 0 1-6.9-4h2.5A6 6 0 1 0 9.6 8H7.1A8 8 0 0 1 14 4zm0 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8z"/>"#),
        "icons/camera.svg" => s!(r#"<rect x="2" y="6" width="14" height="12" rx="2"/><path d="m16 10 6-3v10l-6-3"/>"#),
        "icons/deny.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="m5 5 14 14"/>"#),
        "icons/eye-slash.svg" => s!(r#"<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/><path d="M3 3l18 18"/>"#),
        "icons/blur.svg" => s!(r#"<circle cx="12" cy="12" r="9" stroke-dasharray="2 3"/><circle cx="12" cy="12" r="4"/>"#),
        "icons/play.svg" => f!(r#"<path d="M7 4.5v15a1 1 0 0 0 1.5.9l12-7.5a1 1 0 0 0 0-1.8l-12-7.5A1 1 0 0 0 7 4.5z"/>"#),
        "icons/pause.svg" => f!(r#"<path d="M6 4h4v16H6zM14 4h4v16h-4z"/>"#),
        "icons/stop.svg" => f!(r#"<rect x="5" y="5" width="14" height="14" rx="2"/>"#),
        "icons/send-message.svg" => f!(r#"<path d="M3.4 2.6 21 11.2a1 1 0 0 1 0 1.6L3.4 21.4a.8.8 0 0 1-1.1-1L5 13l8-1-8-1-2.7-7.4a.8.8 0 0 1 1.1-1z"/>"#),
        "icons/volume-off.svg" => f!(r#"<path d="M11.4 3.2A1 1 0 0 1 13 4v16a1 1 0 0 1-1.6.8L6.7 17H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h3.7zM15.3 8.3a1 1 0 0 1 1.4 0L19 10.6l2.3-2.3a1 1 0 1 1 1.4 1.4L20.4 12l2.3 2.3a1 1 0 0 1-1.4 1.4L19 13.4l-2.3 2.3a1 1 0 0 1-1.4-1.4l2.3-2.3-2.3-2.3a1 1 0 0 1 0-1.4z"/>"#),
        "icons/voice-low.svg" => f!(r#"<path d="M11.4 3.2A1 1 0 0 1 13 4v16a1 1 0 0 1-1.6.8L6.7 17H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h3.7zM15.2 8.4a1 1 0 0 1 1.4-.2 4.7 4.7 0 0 1 0 7.6 1 1 0 1 1-1.2-1.6 2.7 2.7 0 0 0 0-4.4 1 1 0 0 1-.2-1.4z"/>"#),
        "icons/refresh.svg" => s!(r#"<path d="M20 4v5h-5"/><path d="M19.5 9A8 8 0 0 0 5 7"/><path d="M4 20v-5h5"/><path d="M4.5 15A8 8 0 0 0 19 17"/>"#),
        "icons/image-plus.svg" => s!(r#"<path d="M14 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-9M3 17l5-5 4 4 2-2 7 7M18 2v6M15 5h6"/>"#),
        "icons/palette.svg" => s!(r#"<path d="M12 3a9 9 0 1 0 0 18c1 0 1.5-.7 1.5-1.5 0-.4-.2-.8-.4-1.1-.3-.3-.4-.6-.4-1 0-.8.7-1.4 1.5-1.4H16a5 5 0 0 0 5-5c0-4.4-4-8-9-8z"/><circle cx="7.5" cy="11" r="1"/><circle cx="10" cy="7" r="1"/><circle cx="14.5" cy="7" r="1"/>"#),
        "icons/info.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-5M12 8h0"/>"#),
        "icons/warning.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="M12 7v6M12 17h0"/>"#),
        "icons/voice-lock.svg" => f!(r#"<path d="M10.4 4.2A1 1 0 0 1 12 5v14a1 1 0 0 1-1.6.8L6.2 16.5H3a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3.2zM16 7.5V6a2.5 2.5 0 0 1 5 0v1.5h.5a.5.5 0 0 1 .5.5v4a.5.5 0 0 1-.5.5h-6a.5.5 0 0 1-.5-.5V8a.5.5 0 0 1 .5-.5zm1.5 0h2V6a1 1 0 0 0-2 0z"/>"#),
        "icons/discovery-badge.svg" => f!(r#"<circle cx="12" cy="12" r="11"/>"#),
        "icons/discovery-badge-fg.svg" => f!(r#"<path d="M12 6.5 13.6 10.4 17.5 12 13.6 13.6 12 17.5 10.4 13.6 6.5 12 10.4 10.4z"/>"#),
        "icons/lock-unlocked.svg" => f!(r#"<path d="M7 10V7a5 5 0 0 1 9.6-2l-1.8.9A3 3 0 0 0 9 7v3h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V11a1 1 0 0 1 1-1z"/>"#),
        "icons/lock.svg" | "icons/text-lock.svg" | "icons/forum-lock.svg" | "icons/announcement-lock.svg" | "icons/stage-lock.svg" => f!(r#"<path d="M7 10V7a5 5 0 0 1 10 0v3h1a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V11a1 1 0 0 1 1-1zm2 0h6V7a3 3 0 0 0-6 0z"/>"#),
        "icons/rules-channel.svg" => s!(r#"<path d="M18 4H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h11v-3H8a2 2 0 0 0-2 2m12-2V4M8 10l2 2 4-4"/>"#),
        "icons/quests.svg" => s!(r#"<path d="M5 4h14v6a7 7 0 0 1-14 0zM9 21h6M12 17v4M8 9h8"/>"#),
        "icons/note.svg" => s!(r#"<path d="M14 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h7M7 8h6M7 12h8M7 16h4M19 14v8M15 18h8"/>"#),
        "icons/user-check.svg" => s!(r#"<circle cx="11" cy="7" r="4"/><path d="M3 21a8 8 0 0 1 11-7.4M15 18l2 2 4-4"/>"#),
        "icons/user-clock.svg" => s!(r#"<circle cx="11" cy="7" r="4"/><path d="M3 21a8 8 0 0 1 10-7.7"/><circle cx="18" cy="18" r="4"/><path d="M18 16v2l1.2 1.2"/>"#),
        "icons/ribbon.svg" => s!(r#"<circle cx="12" cy="9" r="6"/><path d="M8.5 14 7 22l5-3 5 3-1.5-8"/>"#),
        "icons/orbs.svg" => s!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 6l4 6-4 6-4-6z"/>"#),
        "icons/user-circle.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><circle cx="12" cy="10" r="3.5"/><path d="M6 18.5a8 6 0 0 1 12 0"/>"#),
        "icons/bell-slash.svg" => s!(r#"<path d="M6 16V11a6 6 0 0 1 9.5-4.9M18 11v5l2 2H6M10 21h4M3 3l18 18"/>"#),
        "icons/forum/book-check.svg" => s!(r#"<path d="M5 4a2 2 0 0 1 2-2h12v16H7a2 2 0 0 0-2 2zM5 20a2 2 0 0 0 2 2h12v-4M9 9l2 2 4-4"/>"#),
        "icons/settings/puzzle.svg" => s!(r#"<path d="M5 7h4a2 2 0 1 1 4 0h4v4a2 2 0 1 1 0 4v4H5z"/>"#),
        "icons/settings/webhook.svg" => s!(r#"<circle cx="12" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="18" r="3"/><path d="M10.5 8.6 7.5 15.4M9 18h6M13.5 8.6l3 6.8"/>"#),
        "icons/settings/followed.svg" => s!(r#"<rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="14" width="7" height="7" rx="2"/><path d="M14 10l6-6M14 4v6h6M10 14l-6 6M4 14v6h6"/>"#),
        "icons/settings/circle-x.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="m9 9 6 6M15 9l-6 6"/>"#),
        "icons/settings/arrow-left.svg" => s!(r#"<path d="M20 12H4m6-6-6 6 6 6"/>"#),
        "icons/settings/mobile-phone.svg" => s!(r#"<rect x="6" y="2" width="12" height="20" rx="3"/><path d="M11 18h2"/>"#),
        "icons/settings/vr-headset.svg" => s!(r#"<rect x="2" y="7" width="20" height="11" rx="3"/><path d="M10 18l2-3 2 3"/>"#),
        "icons/settings/laptop-phone.svg" => s!(r#"<rect x="3" y="4" width="13" height="10" rx="2"/><path d="M2 18h12M17 9h4v11h-4z"/>"#),
        "icons/settings/circle-error.svg" => s!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v6m0 3.5v.5"/>"#),
        "icons/settings/shield-user.svg" | "icons/mod/shield-user.svg" => s!(r#"<path d="M12 2 4 5v6c0 5 3.5 9 8 11 4.5-2 8-6 8-11V5z"/><circle cx="12" cy="10" r="2.5"/><path d="M8 16a4 4 0 0 1 8 0"/>"#),
        "icons/settings/slash.svg" => s!(r#"<path d="M16 4 8 20"/>"#),
        "icons/settings/chevron-down.svg" => s!(r#"<path d="m5 9 7 7 7-7"/>"#),
        "icons/settings/eye.svg" => s!(r#"<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#),
        "icons/settings/bold.svg" => s!(r#"<path d="M7 4h6a4 4 0 0 1 0 8H7zM7 12h7a4 4 0 0 1 0 8H7z"/>"#),
        "icons/settings/italic.svg" => s!(r#"<path d="M10 4h8M6 20h8M14 4l-4 16"/>"#),
        "icons/settings/underline.svg" => s!(r#"<path d="M7 4v7a5 5 0 0 0 10 0V4M5 20h14"/>"#),
        "icons/settings/strike.svg" => s!(r#"<path d="M4 12h16M16 6.5A4 4 0 0 0 12 5c-2.5 0-4 1.3-4 3M8 17.5A4 4 0 0 0 12 19c2.5 0 4-1.3 4-3.5"/>"#),
        "icons/settings/synced.svg" => s!(r#"<path d="M20 11a8 8 0 0 0-14.6-4.5M4 13a8 8 0 0 0 14.6 4.5M5 3v4h4M19 21v-4h-4"/>"#),
        "icons/settings/unsynced.svg" => s!(r#"<path d="M20 11a8 8 0 0 0-14.6-4.5M4 13a8 8 0 0 0 14.6 4.5M5 3v4h4M19 21v-4h-4M12 8v4M12 16h0"/>"#),
        "icons/settings/globe.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20"/>"#),
        "icons/settings/clock.svg" => s!(r#"<circle cx="12" cy="12" r="10"/><path d="M12 7v5l3 2"/>"#),
        "icons/settings/quote.svg" => s!(r#"<path d="M5 5v14M10 8h9M10 12h9M10 16h6"/>"#),
        "icons/close-small.svg" => s!(r#"<path d="m7 7 10 10M17 7 7 17"/>"#),
        "icons/plus-medium.svg" => s!(r#"<path d="M12 5v14M5 12h14"/>"#),
        "icons/moderation.svg" => s!(r#"<path d="M12 2 4 6v6c0 5 3.5 8.5 8 10 4.5-1.5 8-5 8-10V6z"/><path d="m9 9 6 6m0-6-6 6"/>"#),
        "icons/maximize.svg" => s!(r#"<path d="M14 4h6v6M10 20H4v-6M20 4l-6 6M4 20l6-6"/>"#),
        "icons/pen.svg" => f!(r#"<path d="M16.3 3.3a2.4 2.4 0 0 1 3.4 0l1 1a2.4 2.4 0 0 1 0 3.4L9.5 18.9 3 21l2.1-6.5zM15 6.4 17.6 9"/>"#),
        "icons/id.svg" => f!(r#"<path fill-rule="evenodd" d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm2 4v8h2V8zm4 0v8h3a4 4 0 0 0 0-8zm2 2h1a2 2 0 0 1 0 4h-1z"/>"#),
        "icons/reply.svg" => s!(r#"<path d="M4 20v-5a6 6 0 0 1 6-6h10M15 4l5 5-5 5"/>"#),
        "icons/file.svg" => f!(r#"<path d="M6 2h8l6 6v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zm7 1.5V9h5.5z"/>"#),
        "icons/image.svg" | "icons/media-nsfw.svg" => s!(r#"<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/>"#),
        "icons/images.svg" => s!(r#"<rect x="6" y="2" width="16" height="14" rx="2"/><path d="M2 7v13a2 2 0 0 0 2 2h13"/><circle cx="11" cy="7" r="1.5"/><path d="m22 12-4-4-8 8"/>"#),
        "icons/phone-hangup.svg" => f!(r#"<path d="M12 9c-3.5 0-6.8 1-9.4 2.9a1.5 1.5 0 0 0-.2 2.2l1.8 2a1.5 1.5 0 0 0 1.9.3l2.4-1.4a1.5 1.5 0 0 0 .7-1.3v-1.6a12 12 0 0 1 5.6 0v1.6c0 .5.3 1 .7 1.3l2.4 1.4a1.5 1.5 0 0 0 1.9-.3l1.8-2a1.5 1.5 0 0 0-.2-2.2C18.8 10 15.5 9 12 9z"/>"#),
        "icons/phone-call.svg" => f!(r#"<path d="M7.4 2.5a1 1 0 0 1 .9.6l1.6 3.6a1 1 0 0 1-.2 1.1L8 9.5a12 12 0 0 0 6.5 6.5l1.7-1.7a1 1 0 0 1 1.1-.2l3.6 1.6a1 1 0 0 1 .6.9V20a2 2 0 0 1-2 2A17.5 17.5 0 0 1 2 4.5a2 2 0 0 1 2-2z"/><path d="M14 4.5a5.5 5.5 0 0 1 5.5 5.5M14 8a2 2 0 0 1 2 2" stroke="black" stroke-width="2" stroke-linecap="round" fill="none"/>"#),
        "icons/video-call.svg" => f!(r#"<rect x="2" y="5" width="13" height="14" rx="3"/><path d="M17 10.2 21.4 7a1 1 0 0 1 1.6.8v8.4a1 1 0 0 1-1.6.8L17 13.8z"/>"#),
        "icons/voice/noise-off.svg" => s!(r#"<path d="M3 10v4m4-8v3m0 4v5m5-14v7m0 5v4m5-14v9m4-5v4M3 3l18 18"/>"#),
        "icons/voice/screen-x.svg" => f!(r#"<path fill-rule="evenodd" d="M2 5a3 3 0 0 1 3-3h14a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3V5Zm6.3.3a1 1 0 0 1 1.4 0L12 7.58l2.3-2.3a1 1 0 1 1 1.4 1.42L13.42 9l2.3 2.3a1 1 0 0 1-1.42 1.4L12 10.42l-2.3 2.3a1 1 0 0 1-1.4-1.42L10.58 9l-2.3-2.3a1 1 0 0 1 0-1.4Z"/><path d="M13 19.5c0 .28.22.5.5.5H15a1 1 0 1 1 0 2H9a1 1 0 1 1 0-2h1.5a.5.5 0 0 0 .5-.5v-2c0-.28.22-.5.5-.5h1c.28 0 .5.22.5.5v2Z"/>"#),
        "icons/add-to-dm.svg" => f!(r#"<circle cx="9" cy="8" r="4"/><path d="M1 20a8 6.5 0 0 1 15.5-2.2V21H1zM19 13a1 1 0 0 1 1 1v3h3a1 1 0 1 1 0 2h-3v3a1 1 0 1 1-2 0v-3h-3a1 1 0 1 1 0-2h3v-3a1 1 0 0 1 1-1z"/>"#),
        "icons/user-square.svg" => f!(r#"<path fill-rule="evenodd" d="M5 2h14a3 3 0 0 1 3 3v14a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3V5a3 3 0 0 1 3-3zm7 3.5a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM6 18.5h12a6 5 0 0 0-12 0z"/>"#),
        "icons/super-reaction.svg" => f!(r#"<path fill-rule="evenodd" d="M11 3a9 9 0 1 0 9 9 1 1 0 0 0-2 0 7 7 0 1 1-7-7 1 1 0 0 0 0-2zM7.5 9.5a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm6 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zM6.5 14a4.5 4.5 0 0 0 9 0zM19 1l1 2.5L22.5 4.5 20 5.5 19 8l-1-2.5-2.5-1L18 3.5z"/>"#),
        "icons/user-profile.svg" => f!(r#"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm0 4a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zm-6 12.2a8 8 0 0 0 12 0 7 5 0 0 0-12 0z"/>"#),
        "icons/apps.svg" => f!(r#"<rect x="3" y="3" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="2"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2"/>"#),
        _ => return crate::ui::chat::icons::svg(path).or_else(|| crate::ui::voice_panel::svg_asset(path)).or_else(|| crate::ui::inbox::svg_asset(path)).or_else(|| crate::ui::pickers::svg_asset(path)).or_else(|| crate::ui::call_view::svg_asset(path)),
    })
}

pub fn shortcut_icon(name: &'static str, size: f32) -> gpui::Div {
    use gpui::{ParentElement, Styled};
    use crate::theme::{c, generated::idx, tok, z};

    let icon = match name {
        "events" => "calendar",
        "boosts" | "welcome-boost" => "boost",
        "welcome-invite" => "invite",
        "welcome-icon" => "image-plus",
        "welcome-message" => "chat",
        "welcome-app" => "game-controller",
        other => other,
    };
    gpui::div().size(z(size)).flex_none().flex().items_center().justify_center()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(size)).flex_none().text_color(c(tok(idx::ICON_MUTED))))
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "icon.png" {
            return Ok(Some(Cow::Borrowed(APP_ICON_PNG)));
        }
        if path == "nitro-member-gradient.svg" {
            return Ok(Some(Cow::Borrowed(br##"<svg xmlns="http://www.w3.org/2000/svg" width="696" height="461"><defs><linearGradient id="nitro"><stop stop-color="#8645bd"/><stop offset=".52" stop-color="#af3daf"/><stop offset="1" stop-color="#a85d86"/></linearGradient></defs><rect width="696" height="461" rx="16" fill="url(#nitro)"/></svg>"##)));
        }
        if let Some(p) = path.strip_prefix("local/") {
            return Ok(std::fs::read(p).ok().map(Cow::Owned));
        }
        if let Some(svg) = crate::ui::squircle::svg(path) {
            return Ok(Some(Cow::Owned(svg.into_bytes())));
        }
        if let Some(svg) = crate::ui::chat::video_player::icon_svg(path) {
            return Ok(Some(Cow::Owned(svg.into_bytes())));
        }
        if let Some(svg) = crate::ui::status_svg::svg(path) {
            return Ok(Some(Cow::Owned(svg.into_bytes())));
        }
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(crate::icons::init);
        if let Some(b) = crate::icons::lookup(path) {
            return Ok(Some(Cow::Owned(b.to_vec())));
        }
        Ok(svg_for(path).map(|s| Cow::Borrowed(s.as_bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ALL
            .iter()
            .map(|i| i.path())
            .filter(|p| p.starts_with(path))
            .collect())
    }
}

const ALL: &[Icon] = &[
    Icon::Hash, Icon::Speaker, Icon::Megaphone, Icon::Forum, Icon::Stage, Icon::Chevron,
    Icon::Plus, Icon::Mic, Icon::MicOff, Icon::Headphones, Icon::HeadphonesOff, Icon::Gear,
    Icon::Bell, Icon::Pin, Icon::Members, Icon::Search, Icon::Inbox, Icon::Help, Icon::Gift,
    Icon::Gif, Icon::Sticker, Icon::Emoji, Icon::PlusCircle, Icon::Compass, Icon::Download,
    Icon::Close, Icon::Threads, Icon::Friends, Icon::Nitro, Icon::Lock, Icon::Reply, Icon::File,
    Icon::Image, Icon::PhoneHangup, Icon::Quests, Icon::Shop, Icon::Envelope, Icon::Clyde,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_icon_has_svg() {
        for i in ALL {
            assert!(svg_for(&i.path()).is_some(), "{i:?}");
        }
    }
}
