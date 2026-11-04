# SurSolver for Rust — captcha solver API client

Solve **reCAPTCHA v3**, **Cloudflare Turnstile**, **Cloudflare Challenge**, **GeeTest**,
**DataDome**, **AWS WAF**, **Akamai**, **Kasada** and image captchas from Rust with the official
[SurSolver](https://sursolver.com) client. One `.await` submits the captcha and returns the token
or cookie. Async on Tokio, TLS through rustls.

```powershell
cargo add sursolver serde_json
cargo add tokio --features macros,rt-multi-thread
```

## Quick start

```rust
use serde_json::json;
use sursolver::{Client, task_types};

const API_KEY: &str = "YOUR_API_KEY";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

#[tokio::main]
async fn main() -> Result<(), sursolver::Error> {
    let client = Client::new(API_KEY)?;
    let result = client
        .solve(&json!({
            "type": task_types::TURNSTILE_TOKEN_PROXY_LESS,
            "websiteURL": "https://example.com/login",
            "websiteKey": "0x4AAAAAAA...",
            "userAgent": USER_AGENT,
        }))
        .await?;
    println!("{}", result.solution["token"]);
    Ok(())
}
```

Send the same `userAgent` with the request that uses the result. Tokens and cookies are
single-use, so use them right away. `Client` is cheap to clone and shares one connection pool;
the examples below reuse `client` and `USER_AGENT`.

## Examples

### Solve reCAPTCHA v3 in Rust

```rust
let result = client
    .solve(&json!({
        "type": task_types::RE_CAPTCHA_V3_TOKEN_PROXY_LESS,
        "websiteURL": "https://example.com/login",
        "websiteKey": "6Le-wvkS...",
        "pageAction": "login",
        "userAgent": USER_AGENT,
    }))
    .await?;
println!("{}", result.solution["gRecaptchaResponse"]);
```

For reCAPTCHA v3 Enterprise use `task_types::RE_CAPTCHA_V3_ENTERPRISE_TOKEN_PROXY_LESS`.

### Solve Cloudflare Turnstile

```rust
let result = client
    .solve(&json!({
        "type": task_types::TURNSTILE_TOKEN_PROXY_LESS,
        "websiteURL": "https://example.com/signup",
        "websiteKey": "0x4AAAAAAA...",
        "userAgent": USER_AGENT,
    }))
    .await?;
// Submit the token as the cf-turnstile-response form field.
println!("{}", result.solution["token"]);
```

### Bypass the Cloudflare Challenge page (cf_clearance)

```rust
let result = client
    .solve(&json!({
        "type": task_types::CLOUDFLARE_CHALLENGE_COOKIE,
        "websiteURL": "https://example.com/protected",
        "proxy": "http://user:password@203.0.113.10:8000",
        "userAgent": USER_AGENT,
    }))
    .await?;
println!("{}", result.solution["cf_clearance"]);
```

Replay the cookie through the same proxy and with the same `userAgent`.

### Solve GeeTest v4

```rust
let result = client
    .solve(&json!({
        "type": task_types::GEE_TEST_TOKEN_PROXY_LESS,
        "websiteURL": "https://example.com/login",
        "captchaId": "e392e1d7fd421dc63325744d5a2b9c73",
        "userAgent": USER_AGENT,
    }))
    .await?;
println!("{} {}", result.solution["lot_number"], result.solution["pass_token"]);
```

GeeTest v3: send `gt` and a fresh `challenge` instead of `captchaId`.

### Solve the DataDome slider

```rust
let result = client
    .solve(&json!({
        "type": task_types::DATA_DOME_SLIDER_COOKIE,
        "websiteURL": "https://example.com/",
        "captchaUrl": "https://geo.captcha-delivery.com/captcha/?initialCid=...",
        "proxy": "http://user:password@203.0.113.10:8000",
        "userAgent": USER_AGENT,
    }))
    .await?;
println!("{}", result.solution["datadome"]);
```

### Get an AWS WAF token (silent challenge)

```rust
let result = client
    .solve(&json!({
        "type": task_types::AWS_CHALLENGE_COOKIE,
        "websiteURL": "https://example.com/",
        "scriptUrl": "https://example.token.awswaf.com/example/challenge.js",
        "userAgent": USER_AGENT,
    }))
    .await?;
println!("{}", result.solution["aws-waf-token"]);
```

The visual AWS WAF CAPTCHA is `task_types::AWS_CAPTCHA_COOKIE` and needs your proxy.

### Solve an image captcha (image to text)

```rust
use base64::Engine; // cargo add base64

let image = std::fs::read("captcha.png")?;
let result = client
    .solve(&json!({
        "type": task_types::IMAGE_TO_TEXT_TASK,
        "module": "1",
        "body": base64::engine::general_purpose::STANDARD.encode(image),
    }))
    .await?;
println!("{}", result.solution["answer"]);
```

## Supported captchas

| Captcha | `task.type` | Guide |
|---|---|---|
| reCAPTCHA v3 | `ReCaptchaV3TokenProxyLess` | [recaptcha-v3](https://sursolver.com/solvers/recaptcha-v3) |
| reCAPTCHA v3 Enterprise | `ReCaptchaV3EnterpriseTokenProxyLess` | [recaptcha-v3-enterprise](https://sursolver.com/solvers/recaptcha-v3-enterprise) |
| Cloudflare Turnstile | `TurnstileTokenProxyLess` | [turnstile](https://sursolver.com/solvers/turnstile) |
| Cloudflare Challenge | `CloudflareChallengeCookie` | [cloudflare-challenge](https://sursolver.com/solvers/cloudflare-challenge) |
| GeeTest v3 / v4 | `GeeTestTokenProxyLess` | [geetest](https://sursolver.com/solvers/geetest) |
| DataDome | `DataDomeSliderCookie` | [datadome](https://sursolver.com/solvers/datadome) |
| AWS WAF Challenge | `AwsChallengeCookie` | [aws-challenge](https://sursolver.com/solvers/aws-challenge) |
| AWS WAF CAPTCHA | `AwsCaptchaCookie` | [aws-captcha](https://sursolver.com/solvers/aws-captcha) |
| Akamai (`_abck`) | `AkamaiCookieProxyLess` | [akamai](https://sursolver.com/solvers/akamai) |
| Akamai SBSD | `AkamaiSbsdCookie` | [akamai-sbsd](https://sursolver.com/solvers/akamai-sbsd) |
| Akamai BMP (Android) | `AkamaiBmpTask` | [akamai-bmp](https://sursolver.com/solvers/akamai-bmp) |
| Kasada | `KasadaTokenProxyLess` | [kasada](https://sursolver.com/solvers/kasada) |
| Shape | `ShapeCookieProxyLess` | [shape](https://sursolver.com/solvers/shape) |
| Imperva (reese84 / utmvc) | `ImpervaCookie`, `ImpervaUtmvcCookie` | [imperva](https://sursolver.com/solvers/imperva) |
| Vercel Security Checkpoint | `VercelCookie` | [vercel](https://sursolver.com/solvers/vercel) |
| FriendlyCaptcha | `FriendlyCaptchaTokenProxyLess` | [friendlycaptcha](https://sursolver.com/solvers/friendlycaptcha) |
| MTCaptcha | `MtCaptchaTokenProxyLess` | [mtcaptcha](https://sursolver.com/solvers/mtcaptcha) |
| Prosopo | `ProsopoTokenProxyLess` | [prosopo](https://sursolver.com/solvers/prosopo) |
| CaptchaFox | `CaptchaFoxToken` | [captchafox](https://sursolver.com/solvers/captchafox) |
| Altcha | `AltchaTokenProxyLess` | [altcha](https://sursolver.com/solvers/altcha) |
| Lemin | `LeminTokenProxyLess` | [lemin](https://sursolver.com/solvers/lemin) |
| Castle (Android) | `CastleAndroidToken` | [castle-android](https://sursolver.com/solvers/castle-android) |
| Image to Text | `ImageToTextTask` | [image-to-text](https://sursolver.com/solvers/image-to-text) |

Each guide lists the fields to send and where to find them on the page. The live list is
`client.list_task_types().await`; prices are `client.list_services().await`.

## Errors

```rust
use sursolver::ApiErrorKind;

match client.solve(&task).await {
    Ok(result) => println!("{}", result.solution["token"]),
    Err(error) => match error.api_kind() {
        Some(ApiErrorKind::NoSlotAvailable) => {} // all your threads are busy: try again in a moment
        Some(ApiErrorKind::TaskFailed) => {} // this captcha could not be solved; its charge is refunded
        _ => return Err(error.into()),
    },
}
```

Other errors: `ApiErrorKind::Authentication` (wrong API key), `ApiErrorKind::InvalidTask` (a field
is missing or wrong), `ApiErrorKind::InsufficientBalance` (top up), `Error::SolveTimeout` (no
result in time). Cancel a call by dropping its future, for example with `tokio::time::timeout`.

## Links

- [Documentation](https://sursolver.com/docs) and [API reference](https://sursolver.com/docs/api)
- [Proxies guide](https://sursolver.com/docs/guides/proxies) and [error codes](https://sursolver.com/docs/guides/errors)
- [Pricing](https://sursolver.com/pricing)

## License

MIT
