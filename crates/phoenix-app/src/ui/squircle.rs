use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use futures::AsyncReadExt;
use gpui::{App, AppContext as _, Global, ImageCacheError, RenderImage, SharedString, Window};

pub const PATH: &str = "M0 0.464C0 0.301585 0 0.220377 0.0316081 0.158343C0.0594114 0.103776 0.103776 0.0594114 0.158343 0.0316081C0.220377 0 0.301585 0 0.464 0H0.536C0.698415 0 0.779623 0 0.841657 0.0316081C0.896224 0.0594114 0.940589 0.103776 0.968392 0.158343C1 0.220377 1 0.301585 1 0.464V0.536C1 0.698415 1 0.779623 0.968392 0.841657C0.940589 0.896224 0.896224 0.940589 0.841657 0.968392C0.779623 1 0.698415 1 0.536 1H0.464C0.301585 1 0.220377 1 0.158343 0.968392C0.103776 0.940589 0.0594114 0.896224 0.0316081 0.841657C0 0.779623 0 0.698415 0 0.536V0.464Z";

pub const SVG_PATH: &str = "shapes/squircle.svg";

pub fn svg(path: &str) -> Option<String> {
    (path == SVG_PATH).then(|| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><path fill="white" d="{PATH}"/></svg>"#))
}

pub fn shape() -> gpui::Svg {
    gpui::svg().path(SVG_PATH)
}

fn numbers(s: &str) -> Vec<f64> {
    s.split([' ', ',']).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect()
}

pub fn outline(steps: usize) -> Vec<(f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut cur = (0., 0.);
    let mut cmd = ' ';
    let mut buf = String::new();
    let flush = |cmd: char, buf: &str, cur: &mut (f64, f64), pts: &mut Vec<(f64, f64)>| {
        let n = numbers(buf);
        match cmd {
            'M' | 'L' => {
                for p in n.as_chunks::<2>().0 {
                    *cur = (p[0], p[1]);
                    pts.push(*cur);
                }
            }
            'H' => {
                for x in n {
                    *cur = (x, cur.1);
                    pts.push(*cur);
                }
            }
            'V' => {
                for y in n {
                    *cur = (cur.0, y);
                    pts.push(*cur);
                }
            }
            'C' => {
                for c in n.as_chunks::<6>().0 {
                    let (p0, p1, p2, p3) = (*cur, (c[0], c[1]), (c[2], c[3]), (c[4], c[5]));
                    for k in 1..=steps {
                        let t = k as f64 / steps as f64;
                        let u = 1. - t;
                        let a = u * u * u;
                        let b = 3. * u * u * t;
                        let d = 3. * u * t * t;
                        let e = t * t * t;
                        pts.push((a * p0.0 + b * p1.0 + d * p2.0 + e * p3.0, a * p0.1 + b * p1.1 + d * p2.1 + e * p3.1));
                    }
                    *cur = p3;
                }
            }
            _ => {}
        }
    };
    for ch in PATH.chars() {
        if ch.is_ascii_alphabetic() {
            flush(cmd, &buf, &mut cur, &mut pts);
            buf.clear();
            cmd = ch;
        } else {
            buf.push(ch);
        }
    }
    flush(cmd, &buf, &mut cur, &mut pts);
    pts
}

fn span_at(poly: &[(f64, f64)], y: f64) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for (&a, &b) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
            let x = a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0);
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

fn inside(poly: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut odd = false;
    for (&a, &b) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
            let cx = a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if x < cx {
                odd = !odd;
            }
        }
    }
    odd
}

fn near(poly: &[(f64, f64)], x: f64, y: f64, r: f64) -> bool {
    poly.iter().zip(poly.iter().cycle().skip(1)).any(|(&a, &b)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx * dx + dy * dy;
        let t = if len > 0. { (((x - a.0) * dx + (y - a.1) * dy) / len).clamp(0., 1.) } else { 0. };
        let (px, py) = (a.0 + t * dx - x, a.1 + t * dy - y);
        px * px + py * py <= r * r
    })
}

pub fn notch_px(size: u32) -> Option<u32> {
    match size {
        16 => Some(3),
        20 => Some(4),
        24 => Some(5),
        _ => None,
    }
}

pub fn notched_coverage(px: u32, css: u32) -> Vec<f32> {
    const SUB: u32 = 6;
    let Some(n) = notch_px(css) else { return coverage(px) };
    let n_px = px.max(1) as usize;
    let poly = outline(24);
    let shift = f64::from(css - n) / f64::from(css);
    let cut: Vec<(f64, f64)> = poly.iter().map(|&(x, y)| (x + shift, y)).collect();
    let half = 2. / f64::from(css);
    let mut out = vec![0f32; n_px * n_px];
    for y in 0..n_px {
        for x in 0..n_px {
            let mut hit = 0u32;
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let fx = (x as f64 + (f64::from(sx) + 0.5) / f64::from(SUB)) / n_px as f64;
                    let fy = (y as f64 + (f64::from(sy) + 0.5) / f64::from(SUB)) / n_px as f64;
                    if inside(&poly, fx, fy) && !inside(&cut, fx, fy) && !near(&cut, fx, fy, half) {
                        hit += 1;
                    }
                }
            }
            out[y * n_px + x] = hit as f32 / (SUB * SUB) as f32;
        }
    }
    out
}

pub fn coverage(px: u32) -> Vec<f32> {
    const ROWS: u32 = 8;
    let n = px.max(1) as usize;
    let poly = outline(24);
    let mut out = vec![0f32; n * n];
    for y in 0..n {
        for sub in 0..ROWS {
            let fy = (y as f64 + (f64::from(sub) + 0.5) / f64::from(ROWS)) / n as f64;
            let Some((l, r)) = span_at(&poly, fy) else { continue };
            let (l, r) = (l * n as f64, r * n as f64);
            for x in 0..n {
                let cov = (r.min(x as f64 + 1.) - l.max(x as f64)).clamp(0., 1.);
                out[y * n + x] += (cov / f64::from(ROWS)) as f32;
            }
        }
    }
    out
}

pub fn bake(bytes: &[u8], px: u32, notch: Option<u32>) -> Option<RenderImage> {
    let px = px.clamp(1, 1024);
    let src = image::load_from_memory(bytes).ok()?.into_rgba8();
    if src.width() == 0 || src.height() == 0 {
        return None;
    }
    let sized = if src.width() == px && src.height() == px { src } else { image::imageops::resize(&src, px, px, image::imageops::FilterType::Triangle) };
    let mask = match notch {
        Some(css) => notched_coverage(px, css),
        None => coverage(px),
    };
    let mut out = image::RgbaImage::new(px, px);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let [r, g, b, a] = sized.get_pixel(x, y).0;
        let m = mask[(y * px + x) as usize];
        *p = image::Rgba([b, g, r, (f32::from(a) * m).round().clamp(0., 255.) as u8]);
    }
    Some(RenderImage::new(vec![image::Frame::new(out)]))
}

enum State {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

#[derive(Default)]
struct Store {
    images: HashMap<(SharedString, u32, Option<u32>), (State, Instant)>,
}

impl Global for Store {}

const MAX_CACHED: usize = 256;

fn load(url: &SharedString, px: u32, notch: Option<u32>, cx: &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let key = (url.clone(), px, notch);
    let store = cx.default_global::<Store>();
    if let Some((st, used)) = store.images.get_mut(&key) {
        *used = Instant::now();
        return match st {
            State::Loading => None,
            State::Ready(img) => Some(Ok(img.clone())),
            State::Failed => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("squircle decode failed"))))),
        };
    }
    let evicted = if store.images.len() >= MAX_CACHED
        && let Some(old) = store.images.iter().filter(|(_, (st, _))| !matches!(st, State::Loading)).min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone())
    {
        match store.images.remove(&old) {
            Some((State::Ready(img), _)) => Some(img),
            _ => None,
        }
    } else {
        None
    };
    if let Some(img) = evicted {
        cx.drop_image(img, None);
    }
    let store = cx.default_global::<Store>();
    if url.starts_with('/') {
        let state = std::fs::read(url.as_ref()).ok().and_then(|b| bake(&b, px, notch)).map(|i| State::Ready(Arc::new(i))).unwrap_or(State::Failed);
        let res = match &state {
            State::Ready(img) => Some(Ok(img.clone())),
            _ => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("squircle local decode failed"))))),
        };
        store.images.insert(key, (state, Instant::now()));
        return res;
    }
    store.images.insert(key.clone(), (State::Loading, Instant::now()));
    let http = cx.http_client();
    let fetch = url.clone();
    cx.spawn(async move |cx| {
        let mut bytes = Vec::new();
        let ok = match http.get(&fetch, Default::default(), true).await {
            Ok(mut r) => r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let img = if ok { cx.background_spawn(async move { bake(&bytes, px, notch) }).await } else { None };
        cx.update(|cx| {
            if let Some(e) = cx.default_global::<Store>().images.get_mut(&key) {
                e.0 = match img {
                    Some(i) => State::Ready(Arc::new(i)),
                    None => State::Failed,
                };
            }
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

pub fn image(url: SharedString, size: f32) -> gpui::Img {
    gpui::img(move |window: &mut Window, cx: &mut App| {
        let px = (size * crate::theme::zoom() * window.scale_factor()).round().max(1.) as u32;
        load(&url, px, None, cx)
    })
}

pub fn notched_image(url: SharedString, size: f32) -> gpui::Img {
    gpui::img(move |window: &mut Window, cx: &mut App| {
        let px = (size * crate::theme::zoom() * window.scale_factor()).round().max(1.) as u32;
        load(&url, px, Some(size.round() as u32), cx)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_follows_the_client_path() {
        let p = outline(8);
        assert_eq!(p.first(), Some(&(0., 0.464)));
        assert!(p.iter().all(|(x, y)| (0. ..=1.).contains(x) && (0. ..=1.).contains(y)));
        assert!(p.contains(&(0.536, 0.)) && p.contains(&(1., 0.536)));
    }

    #[test]
    fn coverage_is_full_inside_and_empty_in_the_corners() {
        let m = coverage(48);
        assert_eq!(m[24 * 48 + 24], 1.);
        assert_eq!(m[0], 0.);
        assert_eq!(m[47], 0.);
        assert!(m[24] > 0.99);
        assert!(m[48 * 2 + 2] < 0.05);
        let sum: f32 = m.iter().sum();
        let area = sum / (48. * 48.);
        assert!((0.91..0.94).contains(&area), "{area}");
    }

    #[test]
    fn notch_cuts_the_right_edge_like_the_client_mask() {
        let full = coverage(48);
        let m = notched_coverage(48, 16);
        assert_eq!(m[24 * 48 + 12], 1.);
        assert_eq!(m[24 * 48 + 46], 0.);
        assert!(full[24 * 48 + 46] > 0.99);
        assert!(m[24 * 48 + 36] < 0.01);
        assert!(m[24 * 48 + 30] > 0.99);
        assert_eq!(notch_px(20), Some(4));
        assert_eq!(notch_px(32), None);
    }

    #[test]
    fn bake_masks_the_corners() {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]))).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let img = bake(&png, 40, None).unwrap();
        let bytes = img.as_bytes(0).unwrap();
        assert_eq!(bytes[3], 0);
        let mid = (20 * 40 + 20) * 4;
        assert_eq!(&bytes[mid..mid + 4], &[0, 0, 255, 255]);
    }
}
