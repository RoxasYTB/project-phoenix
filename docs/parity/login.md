# Login page and QR block (28/11)

Reference: the desktop client logged out on purpose (user decision, 28/11) and measured over CDP at 1280×700 CSS px,
dpr 1.5; same DOM as `https://discord.com/login` rendered in an isolated headless Chromium (temporary profile, no input).
Modules: `785161` (QR login, `useAuthWebsocket`), `141979` (`qrLogin_FDXpbQ` classes), `498206` (`authBox_rN1Ls3`),
`534373` (`characterBackground__dBQf7`), `68281` (`mainLoginContainer_8bJOD9`), field status `715965`; CSS
`461123.05982d946ffe8c79.css`, `332923.8b5cb2a40c7316db.css`, `448400.3c21419a2f308049.css`. Assets are loaded at runtime
from the CDN, never committed: artwork `d8680b1c1576ecc8.svg` (1440×900, `xMinYMin slice`), logo `131c318dd45b7aa4.svg`
(124×24 at 48,48), QR center `dd05fd1ea37e7747.png` (50×50).

| Point | Client | Phoenix | State |
|---|---|---|---|
| Background | `<img class=artwork>` fixed, full window, SVG `slice` | `img(ARTWORK)` cover, gradient fallback while loading | ✅ |
| Logo | `logoWithText` 124×24 at left 48, top 48 | same | ✅ |
| Card | `authBoxExpanded` 784 wide, padding 32, `--modal-background`, radius 8, shadow `0 2px 10px rgba(0,0,0,.2)`, centered | same | ✅ |
| Columns | form 416, gap 64, QR 240, both vertically centered (374 high) | same | ✅ |
| Title / subtitle | 24/32 semibold `text-strong`; 16/20 `text-default`, mt 8; block mt 20 | same | ✅ |
| Labels | `body-md/medium` 16/20 `text-strong`, sentence case (no more uppercase), `*` in `text-feedback-critical` ml 4, mb 8 | same, `^upper` removed from the mappings | ✅ |
| Inputs | e-mail 44 high, password 40 high with an eye button 32 (icon 16, `EyeIcon` / `EyeSlashIcon`); bg `input-background-default`, 1 px `input-border-default`, hover `input-border-hover`, focus `input-border-active` + 2 px | same; the eye toggles the mask | ✅ |
| Field error | under the control, gap 4: `CircleErrorIcon` xs + `body-xs/normal` `text-feedback-critical`; error colors on the field | same (`login`/`email` and `password` keys, 988dfc04) | ✅ |
| Forgot / button / footer | link 14/18 `text-link` mt 4 mb 20; primary button 40 high, radius 8, 1 px `rgba(255,255,255,.08)`, 16/20 medium, mb 8; « Need an account? » 14/16 `text-muted` + link, mt 4 | same | ✅ |
| Token login | not in the client | Phoenix link kept under the footer | ➖ Phoenix only |
| QR box | `qrCodeContainer` 176×176 white, radius 4, padding 8, mb 32; QR level M, 37 modules on 160 px, no quiet zone, `image-rendering: pixelated`; logo 50×50 centered | same; level M (was L), modules painted on 160 px (was floored cells and a 28 px app icon on a white tile) | ✅ |
| QR text | 24/32 semibold `text-strong` « Se connecter avec un code QR » (2 lines in 240); `body-md/normal` with bold « application mobile Discord »; « Ou connecte-toi avec une clé d'accès » 14/18 link in a padding-8 stack | same (`Qq+A6i ^bold`, `/kpMDt`) | ✅ (passkey click does nothing: WebAuthn ❌, see below) |
| Loading | `INITIALIZING`: wandering cubes inside the white box | same (`spinner::spinner`) | ✅ |
| Failure / expiry | no error state: socket close, timeout or failure go back to `INITIALIZING` and reconnect with backoff 1.5 s → 30 s; mobile cancel reconnects at once | same: `Unavailable` shows the spinner and retries (`next_backoff`); the raw error text and « Retry » are gone | ✅ |
| Scanned | `PENDING_TICKET`: avatar 120 (mobile online status), « Regarde ton téléphone ! », « Connexion en tant que {username} », « Ce n'est pas moi, recommencer » `brand-500` mt 16 | avatar 120 from the decrypted user record (default avatar when none), texts and start over | ✅ (status dot ❌) |
| Logging in | `PENDING_LOGIN` / `FINISH`: wandering cubes | spinner while the ticket is exchanged | ✅ |

# Login: multi-factor authentication (A2F)

Reference: client JS of 2026-11 (main bundle `web.b63d846b329d6470.js`, chunk 220813 `cf6841236b599214.js` = MFASlides,
chunk `2c265dc1fd64ed7d.js` = login view, fetched by GET from the CDN), CSS `715528.efaa5bc3830d24bf.css` (`*_b5g7JS`)
and `332923.8b5cb2a40c7316db.css` (`authBox_rN1Ls3`), strings from `en-US` / `fr` bundles. No real login was made:
everything is checked against a local HTTP server (phoenix-api tests) and the MockBackend.

## Flow

| Point | Client | Phoenix | State |
|---|---|---|---|
| `POST /auth/login` → `mfa: true` | `LOGIN_MFA_STEP` keeps `ticket`, `totp`, `sms`, `backup`, `webauthn` (challenge), `login_instance_id` | `LoginResponse::MfaRequired` (+ `login_instance_id`), no error line | ✅ |
| Methods order | `ea()`: webauthn, totp, backup, sms; first slide = `methods[0]`, else `select` | `MfaOptions::methods`, `MfaFlow::new` | ✅ |
| Validation | `loginMFAv2`: `POST /auth/mfa/{totp,sms,backup,webauthn}` body `{code, ticket, login_source, gift_code_sku_id, login_instance_id}` | `RestClient::login_mfa` (was: backup sent to `/auth/mfa/totp`) | ✅ |
| Token | `LOGIN_SUCCESS` | `connect(.., remember = true)`, keyring like the password login | ✅ |
| SMS | slide mount sends `POST /auth/mfa/sms/send {ticket}`; subtitle « Sending... » then « We've sent a message to {phone}… »; « Resend SMS? » button | `auth::send_sms` on entering the slide, resend button | ✅ (never sent for real) |
| Backup code | warning slide « Slow down! Are you sure? », 10 s countdown (bold number), then « I'm sure, use a backup code » → code slide; `-` removed before sending | `MfaSlide::Backup` / `BackupAck`, `BACKUP_WAIT` | ✅ |
| Limits | TOTP `maxLength 6`, confirm only at 6; SMS `maxLength 10`, confirm at 6; backup `maxLength 11`, confirm from 8 | `MfaSlide::max_len`, `can_submit`, `clamp_code` (a paste is cut like the browser `maxLength`) | ✅ |
| Enter | form submit, blocked while Confirm is disabled | PressEnter → `submit_mfa`, ignored if `payload` is `None` | ✅ |
| Focus | input focused when its slide is ready | `auth::sync` focuses on slide change | ✅ (no slide animation) |
| Back | header close button → `LOGIN_RESET` (back to the e-mail form, new QR) | `auth::go_back`, `Backend::reset_mfa` | ✅ |
| Errors | `e.body?.message ?? e.message` under the field: « Invalid two-factor code » (60008), expired ticket (60006, server text), rate limit, captcha via the HTTP layer | `account::mfa_err`: server message; 429 and captcha as for the password login; otherwise « An error occurred. Please try again. » | ✅ (captcha: Phoenix cannot show hCaptcha, explicit message) |
| Security key | slide with « Authenticate with a passkey or security key »; `navigator.credentials.get` with the challenge, then `/auth/mfa/webauthn` | slide and button shown; click shows « An error occurred. Please try again. » (client text for an API failure) | ❌ WebAuthn (CTAP2/FIDO over hidraw, or a platform authenticator) not implemented |
| QR login | no MFA (the phone already authenticated) | unchanged; QR socket closed while the MFA box is shown and reopened after « back », like the client unmount | ✅ |

## Visual values

- Box: `AuthBox` with `style={padding:0}`: width 480, `--modal-background`, `--radius-sm` 8 px, `--legacy-elevation-high`
  `0 2px 10px 0 black 20%`. The 784 px login box is replaced, as in the client `switch(loginStatus)`.
- `.header_b5g7JS`: padding 24 24 0; vertical stack gap 8: title `heading-xl/semibold` `text-strong`
  « Multi-Factor Authentication », optional subtitle `heading-lg/semibold`.
- `.closeButton_b5g7JS`: absolute, top 12, inline-end 12 (icon-only `XLargeIcon`, 40 px, icon 20).
- `.content_b5g7JS`: flex column, padding 24. `.error_b5g7JS`: margin-top 8, `body-sm/normal`, `text-feedback-critical`.
- `.footer_b5g7JS`: padding 0 24 24. `ButtonGroup fullWidth` gap 8; it turns vertical when a label overflows, which is
  the case for « Verify with something else » / « Vérifier avec autre chose » next to « Confirm » at 432 px, so Phoenix
  always stacks the buttons (secondary first, as in the DOM order).
- `.listItemContainer_b5g7JS`: full width, `--background-base-low`, 1 px `--border-subtle`, radius 8, margin-bottom 8,
  padding-inline-start 8, hover `--interactive-background-hover`, active `--interactive-background-active`;
  text `body-md/semibold` margin-start 8; arrow `ChevronSmallRightIcon` 20, `--interactive-text-default`, height 50,
  margin-inline auto 16.
- Strings: `saHocI`, `XQaT+8`, `Tot4EC`, `cY+Oob`, `HZPBOd`, `tARzgo`, `C/ZAw/`, `fZSi1D`, `c5J7O0`, `Nixbli`, `tsWkAE`,
  `GK/Qfd`, `oyjspn`, `LQdCQE`, `8r6h7+`, `ZF29L6`, `Xr3Eks`, `xSCvBf`, method labels `gTH4Dp` `nXKmyf` `ZbVwZW` `vhSRKf`
  (en-US and fr, keys `login_mfa.*`). The English SMS text has two spaces after the number; HTML shows one.

## Not reproduced

- The `Slides` transition between slides (width 100 %, spring); slides switch instantly.
- `B.M` (module 532446, not in the cache) wrapping the SMS field and « Resend SMS? »: drawn as a row, button aligned
  to the bottom of the field. To check when the module is available.
- Button `loading` uses the Phoenix three-dot spinner.
- No dump of the real screen: it cannot be opened without logging out of the user's account.

## Tests

- `phoenix-api` `rest::auth::tests`: login → MFA shape, TOTP / SMS / backup routes and bodies, SMS send, 60008 / 60006 /
  captcha errors, against a local HTTP server.
- `phoenix-app` `auth::tests` (order, limits, payloads, slide reset), `discord_backend::account::mfa_tests`
  (error texts), `mock::tests::mock_mfa_walks_the_login_flow`.
- Demo renders: `views out.png dark login-mfa|login-mfa-select|login-mfa-backup|login-mfa-sms|login-mfa-error|...`
  (accounts `totp@phoenix.demo`, `mfa@phoenix.demo`, `sms@phoenix.demo`, `key@phoenix.demo`; code `123456`, backup
  `abcd1234`).

## Password login errors, captcha and QR (28/11)

Measured in the cached `web.55063002a61599e3.js` (`AuthenticationActionCreators.login`, `interceptResponse`, captcha
helper `extractCaptchaPropsFromResponse`):

- `POST /auth/login` body is `{login, password, undelete, login_source, gift_code_sku_id}` with `oldFormErrors: true`.
  Phoenix already sent exactly this. The « Corps de formulaire non valide » the user saw was the server's localized
  top-level `message` of a 50035 (`Invalid Form Body`). The real reason sits in `errors.login._errors[0].message` and
  `errors.password._errors[0].message` (`INVALID_LOGIN`, « Identifiant ou mot de passe invalide. »), which the client
  shows next to each field. ✅ Phoenix now reads those per field (`ApiErrorBody::field_error`, `login_err`), red
  label + `--border-feedback-critical` border (`.inputError_rN1Ls3`), and only falls back to the message when there is
  no field error.
- Logged-out requests carry `X-Fingerprint` from `GET /experiments` (`fingerprint` field). ✅ `ensure_fingerprint`
  before password and QR login.
- Captcha: any `400` with `captcha_key` is answered by the client with the hCaptcha modal
  (`captcha_service`, `captcha_sitekey`, `captcha_session_id`, `captcha_rqdata`, `captcha_rqtoken`,
  `should_serve_invisible`), then the same request is replayed with `X-Captcha-Key`, `X-Captcha-Rqtoken`,
  `X-Captcha-Session-Id`. ✅ Phoenix serves a one-off page on `127.0.0.1:<random>/<uuid>` with the hCaptcha widget
  (rqdata via `hcaptcha.setData`), opens it in the system browser, takes the token back and replays the request (up to
  3 times) for `/auth/login` and `/users/@me/remote-auth/login` (QR). hCaptcha `checksiteconfig` accepts host
  `127.0.0.1` for Discord's sitekey (`localhost` is refused, so the page uses the IP). Not verified end to end: Discord
  may still reject a token solved on another host; the user tests it.
- No raw `Debug` on screen any more: captcha, rate limit, network and QR failures use `login.*` strings.
- Card: `.authBox_rN1Ls3` `--modal-background`, radius 8 (`--radius-sm`), padding 32, `.authBoxExpanded` 784 px.
  ❌ The login route's own chunk (form layout, background art) is not in the cache (the user is logged in), so the
  rest of the page keeps its earlier layout.
- Tests: `rest::auth::tests` (fingerprint, captcha replay headers, 50035 field errors, QR ticket retry),
  `captcha::tests` (escaping, local page round trip), `account::mfa_tests::invalid_form_body_*`.
  Demo: `views out.png dark login-invalid`.
