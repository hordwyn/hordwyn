//! Browser calls to the REST API (`/api/v1/...`) for what a server function cannot do or
//! does not need to: requests whose cookies live on API paths (the "Is this your account?"
//! ticket is scoped to `/api/v1/auth/oidc`), the break-glass sign-in that sets its own
//! session cookie, the pending gated action of the re-auth round trip (DEC-326), and the
//! phase 3 changes the REST routes already validate, audit and gate (providers, mail,
//! source switch, linked sign-ins, time zone, password reset). Errors come back like those
//! of a server function (`api::error_code`, `api::retry_after_secs`), so the pages and
//! `reauth::gated` treat both alike. Server side (SSR) these calls never run: they belong
//! to event handlers.

use leptos::prelude::ServerFnError;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The stable code for an answer without an error body (a body the extractor refused).
#[must_use]
pub fn status_code(status: u16) -> &'static str {
    match status {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        413 => "too_large",
        422 => "invalid",
        429 => "too_many_attempts",
        _ => "generic",
    }
}

/// The error of an answer: the `ErrorDto` code when the body has one, else by status.
#[must_use]
pub fn answer_error(status: u16, body: &str) -> ServerFnError {
    let dto = serde_json::from_str::<contracts::ErrorDto>(body).ok();
    let code = dto
        .as_ref()
        .map_or_else(|| status_code(status).to_string(), |d| d.code.clone());
    let retry = dto.and_then(|d| d.retry_after_secs);
    ServerFnError::new(crate::api::encode_error(&code, retry))
}

/// The value of a 2xx answer (`None` for an empty body such as 204).
///
/// # Errors
/// A body that is not the expected JSON.
pub fn answer_value<T: DeserializeOwned>(body: &str) -> Result<Option<T>, ServerFnError> {
    if body.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(body)
        .map(Some)
        .map_err(|e| ServerFnError::new(format!("generic;{e}")))
}

/// `method` `/api/v1{path}` with an optional JSON body and the session cookie.
///
/// # Errors
/// The answer's error code (see [`answer_error`]), `generic` when the request failed.
pub async fn call<T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Option<T>, ServerFnError> {
    let (status, text) = fetch(method, &format!("/api/v1{path}"), body).await?;
    if (200..300).contains(&status) {
        answer_value(&text)
    } else {
        Err(answer_error(status, &text))
    }
}

/// [`call`], with an error answer as its whole `ErrorDto`: the message and field the page
/// shows next to the cause (a channel's own answer to a test, the field of a refused sample;
/// R-38). A request that failed is `generic`.
///
/// # Errors
/// The answer's `ErrorDto`.
pub async fn call_dto<T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Option<T>, contracts::ErrorDto> {
    let generic = || contracts::ErrorDto {
        code: "generic".into(),
        message: String::new(),
        retry_after_secs: None,
        field: None,
    };
    let (status, text) = fetch(method, &format!("/api/v1{path}"), body)
        .await
        .map_err(|_| generic())?;
    if (200..300).contains(&status) {
        return answer_value(&text).map_err(|_| generic());
    }
    Err(
        serde_json::from_str::<contracts::ErrorDto>(&text).unwrap_or_else(|_| {
            contracts::ErrorDto {
                code: status_code(status).into(),
                ..generic()
            }
        }),
    )
}

/// [`call`] for answers without a value worth reading.
///
/// # Errors
/// See [`call`].
pub async fn send(method: &str, path: &str, body: Option<Value>) -> Result<(), ServerFnError> {
    call::<Value>(method, path, body).await.map(|_| ())
}

/// The browser's `fetch`. Its future holds `JsValue`s, which are not `Send`; the browser has
/// one thread, so a `SendWrapper` lets actions and resources take it.
#[cfg(feature = "hydrate")]
fn fetch(
    method: &str,
    url: &str,
    body: Option<Value>,
) -> impl std::future::Future<Output = Result<(u16, String), ServerFnError>> + Send {
    send_wrapper::SendWrapper::new(fetch_local(method.to_string(), url.to_string(), body))
}

#[cfg(feature = "hydrate")]
async fn fetch_local(
    method: String,
    url: String,
    body: Option<Value>,
) -> Result<(u16, String), ServerFnError> {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    let failed = |_: JsValue| ServerFnError::new("generic");
    let init = web_sys::RequestInit::new();
    init.set_method(&method);
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    let headers = web_sys::Headers::new().map_err(failed)?;
    headers.set("accept", "application/json").map_err(failed)?;
    if let Some(b) = &body {
        headers
            .set("content-type", "application/json")
            .map_err(failed)?;
        init.set_body(&JsValue::from_str(&b.to_string()));
    }
    init.set_headers(&headers);
    let req = web_sys::Request::new_with_str_and_init(&url, &init).map_err(failed)?;
    let window = web_sys::window().ok_or_else(|| ServerFnError::new("generic"))?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(failed)?
        .dyn_into()
        .map_err(failed)?;
    let text = JsFuture::from(resp.text().map_err(failed)?)
        .await
        .map_err(failed)?
        .as_string()
        .unwrap_or_default();
    Ok((resp.status(), text))
}

#[cfg(not(feature = "hydrate"))]
// the server build never fetches; the stub keeps the hydrate build's async signature
#[allow(clippy::unused_async)]
async fn fetch(
    method: &str,
    url: &str,
    body: Option<Value>,
) -> Result<(u16, String), ServerFnError> {
    let _ = (method, url, body);
    Err(ServerFnError::new("generic"))
}

/// `POST /api/v1{path}` with a file as the raw body (the import upload).
///
/// # Errors
/// See [`call`].
#[cfg(feature = "hydrate")]
pub fn upload<T: DeserializeOwned + 'static>(
    path: &str,
    file: web_sys::File,
) -> impl std::future::Future<Output = Result<Option<T>, ServerFnError>> + Send {
    let url = format!("/api/v1{path}");
    send_wrapper::SendWrapper::new(async move {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;
        let failed = |_: JsValue| ServerFnError::new("generic");
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        let headers = web_sys::Headers::new().map_err(failed)?;
        headers.set("accept", "application/json").map_err(failed)?;
        headers
            .set("content-type", "application/octet-stream")
            .map_err(failed)?;
        init.set_headers(&headers);
        init.set_body(&file);
        let req = web_sys::Request::new_with_str_and_init(&url, &init).map_err(failed)?;
        let window = web_sys::window().ok_or_else(|| ServerFnError::new("generic"))?;
        let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&req))
            .await
            .map_err(failed)?
            .dyn_into()
            .map_err(failed)?;
        let text = JsFuture::from(resp.text().map_err(failed)?)
            .await
            .map_err(failed)?
            .as_string()
            .unwrap_or_default();
        if (200..300).contains(&resp.status()) {
            answer_value(&text)
        } else {
            Err(answer_error(resp.status(), &text))
        }
    })
}

/// The browser's IANA time zone (`Intl.DateTimeFormat().resolvedOptions().timeZone`).
#[must_use]
pub fn browser_time_zone() -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        let options =
            js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new())
                .resolved_options();
        js_sys::Reflect::get(&options, &"timeZone".into())
            .ok()?
            .as_string()
            .filter(|z| !z.is_empty())
    }
    #[cfg(not(feature = "hydrate"))]
    None
}

/// The time zones this browser knows (`Intl.supportedValuesOf("timeZone")`), for the
/// picker's suggestions; empty where the browser has no such list.
#[must_use]
pub fn browser_time_zones() -> Vec<String> {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let Ok(intl) = js_sys::Reflect::get(&js_sys::global(), &"Intl".into()) else {
            return Vec::new();
        };
        let Some(f) = js_sys::Reflect::get(&intl, &"supportedValuesOf".into())
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        else {
            return Vec::new();
        };
        f.call1(&intl, &"timeZone".into())
            .ok()
            .and_then(|a| a.dyn_into::<js_sys::Array>().ok())
            .map(|a| a.iter().filter_map(|v| v.as_string()).collect())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_become_the_codes_of_server_functions() {
        let e = answer_error(
            409,
            r#"{"code":"last_sign_in_method","message":"no way to sign in would remain"}"#,
        );
        assert_eq!(crate::api::error_code(&e), "last_sign_in_method");
        let e = answer_error(
            429,
            r#"{"code":"too_many_attempts","message":"wait","retry_after_secs":600}"#,
        );
        assert_eq!(crate::api::retry_after_secs(&e), Some(600));
        // an extractor's plain-text refusal
        let e = answer_error(422, "Failed to deserialize the JSON body");
        assert_eq!(crate::api::error_code(&e), "invalid");
        assert_eq!(crate::api::error_code(&answer_error(500, "")), "generic");
        let v: Option<Value> = answer_value("").unwrap();
        assert_eq!(v, None);
        let v: Option<Value> = answer_value(r#"{"a":1}"#).unwrap();
        assert_eq!(v, Some(serde_json::json!({"a": 1})));
    }
}
