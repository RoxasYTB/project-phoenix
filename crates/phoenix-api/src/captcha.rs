use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{ApiErrorBody, Error, Result};
use crate::rest::CaptchaSolution;

pub const HCAPTCHA_SCRIPT: &str = "https://js.hcaptcha.com/1/api.js?render=explicit&onload=phxLoad&recaptchacompat=off";
const SOLVE_TIMEOUT: Duration = Duration::from_secs(600);
const MAX_BODY: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub service: String,
    pub sitekey: String,
    pub rqdata: Option<String>,
    pub invisible: bool,
}

impl Challenge {
    pub fn from_body(body: &ApiErrorBody) -> Option<Self> {
        let sitekey = body.captcha_sitekey.clone().filter(|s| !s.is_empty())?;
        Some(Self {
            service: body.captcha_service.clone().unwrap_or_else(|| "hcaptcha".into()),
            sitekey,
            rqdata: body.captcha_rqdata.clone().filter(|s| !s.is_empty()),
            invisible: body.should_serve_invisible,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct PageText {
    pub lang: String,
    pub title: String,
    pub subtitle: String,
    pub done: String,
    pub cancel: String,
}

pub type SolveFuture = Pin<Box<dyn Future<Output = Option<CaptchaSolution>> + Send>>;
pub type Solver = Arc<dyn Fn(ApiErrorBody) -> SolveFuture + Send + Sync>;

pub fn solver<F, Fut>(f: F) -> Solver
where
    F: Fn(ApiErrorBody) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<CaptchaSolution>> + Send + 'static,
{
    Arc::new(move |body| -> SolveFuture { Box::pin(f(body)) })
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()).replace("</", "<\\/")
}

pub fn page(ch: &Challenge, text: &PageText, nonce: &str) -> String {
    let lang = if text.lang.is_empty() { "en" } else { text.lang.as_str() };
    let rqdata = ch.rqdata.as_deref().map(js_str).unwrap_or_else(|| "null".into());
    format!(
        r#"<!doctype html><html lang="{lang_attr}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title}</title>
<style>html,body{{margin:0;height:100%;background:#121214;font-family:"gg sans","Noto Sans",Arial,sans-serif;color:#dbdee1}}
main{{min-height:100%;display:flex;align-items:center;justify-content:center}}
.card{{width:408px;box-sizing:border-box;padding:24px 16px 16px;background:#242429;border-radius:8px;box-shadow:0 8px 16px rgba(0,0,0,.24);text-align:center}}
h1{{margin:0 0 8px;font-size:24px;line-height:30px;font-weight:600;color:#fbfbfb}}
p{{margin:0 0 16px;font-size:16px;line-height:20px;color:#b5bac1}}
#w{{display:flex;justify-content:center;min-height:78px}}
button{{margin-top:16px;background:none;border:0;color:#949ba4;font:inherit;font-size:14px;cursor:pointer}}
button:hover{{text-decoration:underline}}</style></head>
<body><main><div class="card"><h1 id="t">{title}</h1><p id="s">{subtitle}</p><div id="w"></div><button id="c" type="button">{cancel}</button></div></main>
<script>
const N={nonce},RQ={rqdata},DONE={done};
function post(p,b){{return fetch("/"+N+"/"+p,{{method:"POST",headers:{{"content-type":"text/plain"}},body:b||""}})}}
function finish(){{document.getElementById("w").remove();document.getElementById("c").remove();document.getElementById("s").textContent=DONE;setTimeout(()=>window.close(),400)}}
window.phxLoad=function(){{const id=hcaptcha.render("w",{{sitekey:{sitekey},theme:"dark",hl:{lang},size:{size},callback:t=>post("solved",t).then(finish),"error-callback":()=>{{}},"expired-callback":()=>hcaptcha.reset(id)}});if(RQ)hcaptcha.setData(id,{{rqdata:RQ}});if({invisible})hcaptcha.execute(id)}};
document.getElementById("c").onclick=()=>post("cancel").then(()=>window.close());
</script><script src="{script}&amp;hl={lang_attr}" async defer></script></body></html>"#,
        lang_attr = esc_html(lang),
        lang = js_str(lang),
        title = esc_html(&text.title),
        subtitle = esc_html(&text.subtitle),
        cancel = esc_html(&text.cancel),
        done = js_str(&text.done),
        nonce = js_str(nonce),
        sitekey = js_str(&ch.sitekey),
        size = js_str(if ch.invisible { "invisible" } else { "normal" }),
        invisible = ch.invisible,
        script = HCAPTCHA_SCRIPT.replace('&', "&amp;"),
    )
}

enum Outcome {
    Solved(String),
    Cancelled,
    Continue,
}

async fn handle(stream: TcpStream, html: &str, nonce: &str) -> std::io::Result<Outcome> {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    reader.read_line(&mut request).await?;
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut payload = vec![0; len.min(MAX_BODY)];
    reader.read_exact(&mut payload).await?;
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let base = format!("/{nonce}");
    let base_slash = format!("{base}/");
    let solved = format!("{base}/solved");
    let cancel = format!("{base}/cancel");
    let (status, ctype, body, outcome) = match (method.as_str(), path.as_str()) {
        ("GET", p) if p == base || p == base_slash => ("200 OK", "text/html; charset=utf-8", html.to_owned(), Outcome::Continue),
        ("POST", p) if p == solved && len <= MAX_BODY => {
            let token = String::from_utf8_lossy(&payload).trim().to_owned();
            if token.is_empty() {
                ("400 Bad Request", "text/plain", String::new(), Outcome::Continue)
            } else {
                ("204 No Content", "text/plain", String::new(), Outcome::Solved(token))
            }
        }
        ("POST", p) if p == cancel => ("204 No Content", "text/plain", String::new(), Outcome::Cancelled),
        _ => ("404 Not Found", "text/plain", String::new(), Outcome::Continue),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\ncache-control: no-store\r\nreferrer-policy: no-referrer\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let stream = reader.get_mut();
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(outcome)
}

pub struct LocalPage {
    listener: TcpListener,
    nonce: String,
    html: String,
}

impl LocalPage {
    pub async fn bind(ch: &Challenge, text: &PageText) -> Result<Self> {
        if ch.service != "hcaptcha" {
            return Err(Error::Protocol(format!("unsupported captcha service {}", ch.service)));
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| Error::Protocol(format!("captcha listener: {e}")))?;
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let html = page(ch, text, &nonce);
        Ok(Self { listener, nonce, html })
    }

    pub fn url(&self) -> String {
        let port = self.listener.local_addr().map(|a| a.port()).unwrap_or(0);
        format!("http://127.0.0.1:{port}/{}", self.nonce)
    }

    pub async fn wait(self) -> Option<String> {
        let serve = async {
            loop {
                let Ok((stream, _)) = self.listener.accept().await else { return None };
                match tokio::time::timeout(Duration::from_secs(10), handle(stream, &self.html, &self.nonce)).await {
                    Ok(Ok(Outcome::Solved(t))) => return Some(t),
                    Ok(Ok(Outcome::Cancelled)) => return None,
                    _ => {}
                }
            }
        };
        tokio::time::timeout(SOLVE_TIMEOUT, serve).await.ok().flatten()
    }
}

pub async fn solve(body: &ApiErrorBody, text: &PageText, open: impl FnOnce(&str)) -> Option<CaptchaSolution> {
    let ch = Challenge::from_body(body)?;
    let page = LocalPage::bind(&ch, text).await.ok()?;
    open(&page.url());
    let key = page.wait().await?;
    Some(CaptchaSolution::from_challenge(key, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> ApiErrorBody {
        serde_json::from_str(
            r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"b2b02ab5","captcha_service":"hcaptcha","captcha_rqdata":"rq\"</script>","captcha_rqtoken":"tok","captcha_session_id":"sess"}"#,
        )
        .unwrap()
    }

    #[test]
    fn challenge_reads_the_client_fields() {
        let ch = Challenge::from_body(&body()).unwrap();
        assert_eq!(ch.sitekey, "b2b02ab5");
        assert_eq!(ch.rqdata.as_deref(), Some("rq\"</script>"));
        assert!(!ch.invisible);
        assert!(Challenge::from_body(&ApiErrorBody::default()).is_none());
    }

    #[test]
    fn page_escapes_server_strings() {
        let ch = Challenge::from_body(&body()).unwrap();
        let html = page(&ch, &PageText { lang: "fr".into(), title: "<b>".into(), ..Default::default() }, "n");
        assert!(html.contains("&lt;b&gt;"));
        assert!(!html.contains("rq\"</script>"));
        assert!(html.contains("hcaptcha.setData"));
    }

    #[tokio::test]
    async fn local_page_returns_the_solution_and_ignores_strangers() {
        let b = body();
        let ch = Challenge::from_body(&b).unwrap();
        let page = LocalPage::bind(&ch, &PageText::default()).await.unwrap();
        let url = page.url();
        let rest = url.trim_start_matches("http://");
        let (addr, path) = rest.split_once('/').unwrap();
        let addr = addr.to_owned();
        let path = format!("/{path}");
        let waiter = tokio::spawn(page.wait());
        let send = |req: String| {
            let addr = addr.clone();
            async move {
                let mut s = TcpStream::connect(addr).await.unwrap();
                s.write_all(req.as_bytes()).await.unwrap();
                let mut out = String::new();
                s.read_to_string(&mut out).await.unwrap();
                out
            }
        };
        let got = send(format!("GET {path} HTTP/1.1\r\nhost: x\r\n\r\n")).await;
        assert!(got.starts_with("HTTP/1.1 200") && got.contains("b2b02ab5"));
        assert!(send("POST /wrong/solved HTTP/1.1\r\ncontent-length: 3\r\n\r\nbad".into()).await.starts_with("HTTP/1.1 404"));
        send(format!("POST {path}/solved HTTP/1.1\r\ncontent-length: 7\r\n\r\nP1_abcd")).await;
        let key = waiter.await.unwrap().unwrap();
        let sol = CaptchaSolution::from_challenge(key, &b);
        assert_eq!(
            sol.headers(),
            vec![("x-captcha-key", "P1_abcd".to_owned()), ("x-captcha-rqtoken", "tok".to_owned()), ("x-captcha-session-id", "sess".to_owned())]
        );
    }
}
