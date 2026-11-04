// Generated from the SurSolver catalog (GET /v1/task-types) by the spec repository
// tools/templates/crates.ts. Catalog fetched at 2026-11-03T21:16:22.334Z. DO NOT EDIT.

/// Every accepted `task.type` spelling: canonical names, ProxyLess twins and aliases.
pub mod task_types {
    /// `AkamaiBmpTask`
    pub const AKAMAI_BMP_TASK: &str = "AkamaiBmpTask";
    /// `AkamaiCookie`
    pub const AKAMAI_COOKIE: &str = "AkamaiCookie";
    /// `AkamaiCookieProxyLess`
    pub const AKAMAI_COOKIE_PROXY_LESS: &str = "AkamaiCookieProxyLess";
    /// `AkamaiSbsdCookie`
    pub const AKAMAI_SBSD_COOKIE: &str = "AkamaiSbsdCookie";
    /// `AltchaToken`
    pub const ALTCHA_TOKEN: &str = "AltchaToken";
    /// `AltchaTokenProxyLess`
    pub const ALTCHA_TOKEN_PROXY_LESS: &str = "AltchaTokenProxyLess";
    /// `AwsCaptchaCookie`
    pub const AWS_CAPTCHA_COOKIE: &str = "AwsCaptchaCookie";
    /// `AwsChallengeCookie`
    pub const AWS_CHALLENGE_COOKIE: &str = "AwsChallengeCookie";
    /// `CaptchaFoxToken`
    pub const CAPTCHA_FOX_TOKEN: &str = "CaptchaFoxToken";
    /// `CastleAndroidToken`
    pub const CASTLE_ANDROID_TOKEN: &str = "CastleAndroidToken";
    /// `CastleToken`
    pub const CASTLE_TOKEN: &str = "CastleToken";
    /// `CastleWebToken`
    pub const CASTLE_WEB_TOKEN: &str = "CastleWebToken";
    /// `CloudflareChallengeCookie`
    pub const CLOUDFLARE_CHALLENGE_COOKIE: &str = "CloudflareChallengeCookie";
    /// `DataDomeSliderCookie`
    pub const DATA_DOME_SLIDER_COOKIE: &str = "DataDomeSliderCookie";
    /// `FriendlyCaptchaToken`
    pub const FRIENDLY_CAPTCHA_TOKEN: &str = "FriendlyCaptchaToken";
    /// `FriendlyCaptchaTokenProxyLess`
    pub const FRIENDLY_CAPTCHA_TOKEN_PROXY_LESS: &str = "FriendlyCaptchaTokenProxyLess";
    /// `GeeTestToken`
    pub const GEE_TEST_TOKEN: &str = "GeeTestToken";
    /// `GeeTestTokenProxyLess`
    pub const GEE_TEST_TOKEN_PROXY_LESS: &str = "GeeTestTokenProxyLess";
    /// `GeeTestV3Token`
    pub const GEE_TEST_V3_TOKEN: &str = "GeeTestV3Token";
    /// `GeeTestV4Token`
    pub const GEE_TEST_V4_TOKEN: &str = "GeeTestV4Token";
    /// `ImageToText`
    pub const IMAGE_TO_TEXT: &str = "ImageToText";
    /// `ImageToTextTask`
    pub const IMAGE_TO_TEXT_TASK: &str = "ImageToTextTask";
    /// `ImpervaCookie`
    pub const IMPERVA_COOKIE: &str = "ImpervaCookie";
    /// `ImpervaUtmvcCookie`
    pub const IMPERVA_UTMVC_COOKIE: &str = "ImpervaUtmvcCookie";
    /// `KasadaToken`
    pub const KASADA_TOKEN: &str = "KasadaToken";
    /// `KasadaTokenProxyLess`
    pub const KASADA_TOKEN_PROXY_LESS: &str = "KasadaTokenProxyLess";
    /// `LeminToken`
    pub const LEMIN_TOKEN: &str = "LeminToken";
    /// `LeminTokenProxyLess`
    pub const LEMIN_TOKEN_PROXY_LESS: &str = "LeminTokenProxyLess";
    /// `MtCaptchaToken`
    pub const MT_CAPTCHA_TOKEN: &str = "MtCaptchaToken";
    /// `MtCaptchaTokenProxyLess`
    pub const MT_CAPTCHA_TOKEN_PROXY_LESS: &str = "MtCaptchaTokenProxyLess";
    /// `PerimeterXCookie`
    pub const PERIMETER_X_COOKIE: &str = "PerimeterXCookie";
    /// `ProsopoToken`
    pub const PROSOPO_TOKEN: &str = "ProsopoToken";
    /// `ProsopoTokenProxyLess`
    pub const PROSOPO_TOKEN_PROXY_LESS: &str = "ProsopoTokenProxyLess";
    /// `ReCaptchaV2EnterpriseToken`
    pub const RE_CAPTCHA_V2_ENTERPRISE_TOKEN: &str = "ReCaptchaV2EnterpriseToken";
    /// `ReCaptchaV2EnterpriseTokenProxyLess`
    pub const RE_CAPTCHA_V2_ENTERPRISE_TOKEN_PROXY_LESS: &str =
        "ReCaptchaV2EnterpriseTokenProxyLess";
    /// `ReCaptchaV2Token`
    pub const RE_CAPTCHA_V2_TOKEN: &str = "ReCaptchaV2Token";
    /// `ReCaptchaV2TokenProxyLess`
    pub const RE_CAPTCHA_V2_TOKEN_PROXY_LESS: &str = "ReCaptchaV2TokenProxyLess";
    /// `ReCaptchaV3EnterpriseToken`
    pub const RE_CAPTCHA_V3_ENTERPRISE_TOKEN: &str = "ReCaptchaV3EnterpriseToken";
    /// `ReCaptchaV3EnterpriseTokenProxyLess`
    pub const RE_CAPTCHA_V3_ENTERPRISE_TOKEN_PROXY_LESS: &str =
        "ReCaptchaV3EnterpriseTokenProxyLess";
    /// `ReCaptchaV3Token`
    pub const RE_CAPTCHA_V3_TOKEN: &str = "ReCaptchaV3Token";
    /// `ReCaptchaV3TokenProxyLess`
    pub const RE_CAPTCHA_V3_TOKEN_PROXY_LESS: &str = "ReCaptchaV3TokenProxyLess";
    /// `ShapeCookie`
    pub const SHAPE_COOKIE: &str = "ShapeCookie";
    /// `ShapeCookieProxyLess`
    pub const SHAPE_COOKIE_PROXY_LESS: &str = "ShapeCookieProxyLess";
    /// `TurnstileToken`
    pub const TURNSTILE_TOKEN: &str = "TurnstileToken";
    /// `TurnstileTokenProxyLess`
    pub const TURNSTILE_TOKEN_PROXY_LESS: &str = "TurnstileTokenProxyLess";
    /// `VercelCookie`
    pub const VERCEL_COOKIE: &str = "VercelCookie";
}

/// The complete `errorCode` vocabulary of the solver endpoints.
pub mod error_codes {
    /// `ERROR_KEY_DOES_NOT_EXIST`
    pub const ERROR_KEY_DOES_NOT_EXIST: &str = "ERROR_KEY_DOES_NOT_EXIST";
    /// `ERROR_WRONG_CLIENT_KEY`
    pub const ERROR_WRONG_CLIENT_KEY: &str = "ERROR_WRONG_CLIENT_KEY";
    /// `ERROR_ZERO_BALANCE`
    pub const ERROR_ZERO_BALANCE: &str = "ERROR_ZERO_BALANCE";
    /// `ERROR_NO_SLOT_AVAILABLE`
    pub const ERROR_NO_SLOT_AVAILABLE: &str = "ERROR_NO_SLOT_AVAILABLE";
    /// `ERROR_TYPE_NOT_SUPPORTED`
    pub const ERROR_TYPE_NOT_SUPPORTED: &str = "ERROR_TYPE_NOT_SUPPORTED";
    /// `ERROR_TASK_NOT_FOUND`
    pub const ERROR_TASK_NOT_FOUND: &str = "ERROR_TASK_NOT_FOUND";
    /// `ERROR_TASK_EXPIRED`
    pub const ERROR_TASK_EXPIRED: &str = "ERROR_TASK_EXPIRED";
    /// `ERROR_TASK_TIMEOUT`
    pub const ERROR_TASK_TIMEOUT: &str = "ERROR_TASK_TIMEOUT";
    /// `ERROR_FAILED`
    pub const ERROR_FAILED: &str = "ERROR_FAILED";
    /// `ERROR_INVALID_REQUEST`
    pub const ERROR_INVALID_REQUEST: &str = "ERROR_INVALID_REQUEST";
    /// `ERROR_WRONG_USERAGENT`
    pub const ERROR_WRONG_USERAGENT: &str = "ERROR_WRONG_USERAGENT";
    /// `ERROR_INTERNAL_ERROR`
    pub const ERROR_INTERNAL_ERROR: &str = "ERROR_INTERNAL_ERROR";

    /// All codes in catalog order.
    pub const ALL: [&str; 12] = [
        ERROR_KEY_DOES_NOT_EXIST,
        ERROR_WRONG_CLIENT_KEY,
        ERROR_ZERO_BALANCE,
        ERROR_NO_SLOT_AVAILABLE,
        ERROR_TYPE_NOT_SUPPORTED,
        ERROR_TASK_NOT_FOUND,
        ERROR_TASK_EXPIRED,
        ERROR_TASK_TIMEOUT,
        ERROR_FAILED,
        ERROR_INVALID_REQUEST,
        ERROR_WRONG_USERAGENT,
        ERROR_INTERNAL_ERROR,
    ];
}
