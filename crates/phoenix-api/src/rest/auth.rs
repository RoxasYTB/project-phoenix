use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::{Body, CaptchaSolution, RestClient};
use crate::error::Result;

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginMfaType {
    Totp,
    Sms,
    Backup,
    Webauthn,
}

impl LoginMfaType {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::Sms => "sms",
            Self::Backup => "backup",
            Self::Webauthn => "webauthn",
        }
    }
}

pub fn login_mfa_path(kind: LoginMfaType) -> String {
    format!("/auth/mfa/{}", kind.api_name())
}

pub fn login_mfa_body(ticket: &str, code: &str, login_instance_id: Option<&str>) -> serde_json::Value {
    json!({
        "code": code,
        "ticket": ticket,
        "login_source": null,
        "gift_code_sku_id": null,
        "login_instance_id": login_instance_id,
    })
}

impl RestClient {
    pub async fn login_mfa_sms_send(&self, ticket: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct P {
            #[serde(default)]
            phone: String,
        }
        let p: P = self.call(Method::POST, "/auth/mfa/sms/send", Body::Json(json!({ "ticket": ticket }))).await?;
        Ok(p.phone)
    }

    pub async fn login_mfa(&self, kind: LoginMfaType, ticket: &str, code: &str, login_instance_id: Option<&str>) -> Result<String> {
        let t: TokenBody =
            self.call(Method::POST, &login_mfa_path(kind), Body::Json(login_mfa_body(ticket, code, login_instance_id))).await?;
        Ok(t.token)
    }

    pub async fn remote_auth_login(&self, ticket: &str, captcha: Option<&CaptchaSolution>) -> Result<String> {
        #[derive(Deserialize)]
        struct E {
            encrypted_token: String,
        }
        let headers = captcha.map(CaptchaSolution::headers).unwrap_or_default();
        let (_, bytes) = self
            .raw_with(Method::POST, "/users/@me/remote-auth/login", Body::Json(json!({ "ticket": ticket })), 0, &headers)
            .await?;
        let e: E = serde_json::from_slice(&bytes)?;
        Ok(e.encrypted_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientProperties, Error, rest::LoginResponse};
    use serde_json::Value;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    type Seen = Vec<(String, Value, String)>;

    async fn serve(responses: Vec<(&'static str, &'static str)>) -> (RestClient, tokio::task::JoinHandle<Seen>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for (status, response) in responses {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let len = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:")?.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0; len];
                socket.read_exact(&mut body).await.unwrap();
                let line = head.lines().next().unwrap_or_default().to_owned();
                seen.push((line, serde_json::from_slice(&body).unwrap_or(Value::Null), head.to_ascii_lowercase()));
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response}",
                    response.len()
                );
                socket.get_mut().write_all(reply.as_bytes()).await.unwrap();
            }
            seen
        });
        (RestClient::with_base(None, ClientProperties::linux_desktop(), format!("http://{addr}")), server)
    }

    async fn timeout<F: std::future::Future>(f: F) -> F::Output {
        tokio::time::timeout(std::time::Duration::from_secs(5), f).await.unwrap()
    }

    #[tokio::test]
    async fn password_login_then_totp_posts_the_client_body() {
        let (rest, server) = serve(vec![
            (
                "200 OK",
                r#"{"user_id":"1","mfa":true,"sms":true,"ticket":"tkt","backup":true,"totp":true,"webauthn":"{\"publicKey\":{}}","login_instance_id":"inst"}"#,
            ),
            ("200 OK", r#"{"token":"fresh","user_settings":{"locale":"fr","theme":"dark"}}"#),
        ])
        .await;
        let res = timeout(rest.login("a@b.c", "pw")).await.unwrap();
        let LoginResponse::MfaRequired { ticket, totp, sms, backup, webauthn, login_instance_id, .. } = res else { panic!() };
        assert_eq!((ticket.as_str(), totp, sms, backup, webauthn), ("tkt", true, true, true, true));
        assert_eq!(login_instance_id.as_deref(), Some("inst"));
        let token = timeout(rest.login_mfa(LoginMfaType::Totp, &ticket, "123456", login_instance_id.as_deref())).await.unwrap();
        assert_eq!(token, "fresh");
        let seen = server.await.unwrap();
        assert!(seen[0].0.starts_with("POST /auth/login "));
        assert!(seen[1].0.starts_with("POST /auth/mfa/totp "));
        assert_eq!(
            seen[1].1,
            json!({"code":"123456","ticket":"tkt","login_source":null,"gift_code_sku_id":null,"login_instance_id":"inst"})
        );
    }

    #[tokio::test]
    async fn backup_and_sms_use_their_own_routes() {
        let (rest, server) = serve(vec![
            ("200 OK", r#"{"phone":"+*******1234"}"#),
            ("200 OK", r#"{"token":"a"}"#),
            ("200 OK", r#"{"token":"b"}"#),
        ])
        .await;
        assert_eq!(timeout(rest.login_mfa_sms_send("tkt")).await.unwrap(), "+*******1234");
        timeout(rest.login_mfa(LoginMfaType::Sms, "tkt", "654321", None)).await.unwrap();
        timeout(rest.login_mfa(LoginMfaType::Backup, "tkt", "abcd1234", None)).await.unwrap();
        let seen = server.await.unwrap();
        assert!(seen[0].0.starts_with("POST /auth/mfa/sms/send "));
        assert_eq!(seen[0].1, json!({"ticket":"tkt"}));
        assert!(seen[1].0.starts_with("POST /auth/mfa/sms "));
        assert!(seen[2].0.starts_with("POST /auth/mfa/backup "));
        assert_eq!(seen[2].1["code"], "abcd1234");
        assert_eq!(seen[2].1["login_instance_id"], Value::Null);
    }

    #[tokio::test]
    async fn mfa_errors_keep_the_server_message() {
        let (rest, server) = serve(vec![
            ("400 Bad Request", r#"{"message":"Invalid two-factor code","code":60008}"#),
            ("400 Bad Request", r#"{"message":"Invalid two-factor auth ticket","code":60006}"#),
            ("400 Bad Request", r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"site","captcha_service":"hcaptcha"}"#),
        ])
        .await;
        let invalid = timeout(rest.login_mfa(LoginMfaType::Totp, "tkt", "000000", None)).await.unwrap_err();
        assert!(matches!(&invalid, Error::Api { status: 400, body, .. } if body.code == 60008 && body.message == "Invalid two-factor code"));
        let expired = timeout(rest.login_mfa(LoginMfaType::Totp, "old", "123456", None)).await.unwrap_err();
        assert!(matches!(&expired, Error::Api { body, .. } if body.code == 60006));
        let captcha = timeout(rest.login_mfa(LoginMfaType::Totp, "tkt", "123456", None)).await.unwrap_err();
        assert!(matches!(captcha, Error::CaptchaRequired { sitekey: Some(s), .. } if s == "site"));
        assert_eq!(server.await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn captcha_challenge_is_parsed_and_the_retry_carries_the_solution() {
        let (rest, server) = serve(vec![
            ("200 OK", r#"{"fingerprint":"111.abc","assignments":[]}"#),
            (
                "400 Bad Request",
                r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"b2b02ab5","captcha_service":"hcaptcha","captcha_rqdata":"rqd","captcha_rqtoken":"rqt","captcha_session_id":"sid"}"#,
            ),
            ("200 OK", r#"{"user_id":"1","token":"tok"}"#),
        ])
        .await;
        assert_eq!(timeout(rest.ensure_fingerprint()).await.as_deref(), Some("111.abc"));
        let err = timeout(rest.login_with("a@b.c", "pw", None)).await.unwrap_err();
        assert_eq!(err.to_string(), "captcha required");
        let body = err.captcha().unwrap().clone();
        assert_eq!(body.captcha_rqdata.as_deref(), Some("rqd"));
        let sol = CaptchaSolution::from_challenge("P1_key".into(), &body);
        let ok = timeout(rest.login_with("a@b.c", "pw", Some(&sol))).await.unwrap();
        assert!(matches!(ok, LoginResponse::Token { token, .. } if token == "tok"));
        let seen = server.await.unwrap();
        assert!(seen[0].0.starts_with("GET /experiments "));
        assert_eq!(
            seen[1].1,
            json!({"login":"a@b.c","password":"pw","undelete":false,"login_source":null,"gift_code_sku_id":null})
        );
        assert!(seen[1].2.contains("x-fingerprint: 111.abc") && !seen[1].2.contains("x-captcha-key"));
        assert!(seen[2].2.contains("x-captcha-key: p1_key"));
        assert!(seen[2].2.contains("x-captcha-rqtoken: rqt") && seen[2].2.contains("x-captcha-session-id: sid"));
    }

    #[tokio::test]
    async fn invalid_login_keeps_the_field_errors() {
        let (rest, server) = serve(vec![(
            "400 Bad Request",
            r#"{"message":"Invalid Form Body","code":50035,"errors":{"login":{"_errors":[{"code":"INVALID_LOGIN","message":"Login or password is invalid."}]},"password":{"_errors":[{"code":"INVALID_LOGIN","message":"Login or password is invalid."}]}}}"#,
        )])
        .await;
        let err = timeout(rest.login("a@b.c", "pw")).await.unwrap_err();
        let Error::Api { body, .. } = &err else { panic!() };
        assert_eq!(body.code, 50035);
        assert_eq!(body.field_error("login").as_deref(), Some("Login or password is invalid."));
        assert_eq!(body.field_error("password").as_deref(), Some("Login or password is invalid."));
        assert_eq!(body.field_error("email"), None);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn remote_auth_ticket_retries_once_the_captcha_is_solved() {
        let (rest, server) = serve(vec![
            ("400 Bad Request", r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"site","captcha_service":"hcaptcha","captcha_rqtoken":"rqt"}"#),
            ("200 OK", r#"{"encrypted_token":"enc"}"#),
        ])
        .await;
        let solver = crate::captcha::solver(|body| async move { Some(CaptchaSolution::from_challenge("solved".into(), &body)) });
        let enc = timeout(crate::remote_auth::login_ticket(&rest, "tkt", Some(&solver))).await.unwrap();
        assert_eq!(enc, "enc");
        let seen = server.await.unwrap();
        assert!(seen[1].0.starts_with("POST /users/@me/remote-auth/login "));
        assert!(seen[1].2.contains("x-captcha-key: solved") && seen[1].2.contains("x-captcha-rqtoken: rqt"));
        assert_eq!(seen[1].1, json!({"ticket":"tkt"}));
    }

    #[tokio::test]
    async fn remote_auth_without_solver_reports_the_captcha() {
        let (rest, server) = serve(vec![(
            "400 Bad Request",
            r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"site"}"#,
        )])
        .await;
        let err = timeout(crate::remote_auth::login_ticket(&rest, "tkt", None)).await.unwrap_err();
        assert!(err.captcha().is_some());
        server.await.unwrap();
    }

    #[test]
    fn mfa_paths_match_the_client_routes() {
        assert_eq!(login_mfa_path(LoginMfaType::Totp), "/auth/mfa/totp");
        assert_eq!(login_mfa_path(LoginMfaType::Webauthn), "/auth/mfa/webauthn");
    }
}
