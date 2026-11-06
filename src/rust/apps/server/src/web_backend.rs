//! `web::Backend` for the web app's server functions, on top of the same operations as the
//! REST API (`api::ops`).

use api::{ApiError, ApiState, ops};
use async_trait::async_trait;
use contracts::{
    AttributionDto, CompanionDto, ErrorDto, ItemPageDto, LibraryEntryDto, LibraryItemDto, ListDto,
    NewProgressDto, ProgressEventDto, SearchResponseDto, SessionDto, SignInDto, SignUpDto, UserDto,
};
use domain::user::Actor;
use web::backend::{BResult, Backend};

pub struct WebBackend(pub ApiState);

#[allow(clippy::needless_pass_by_value)]
fn err(e: ApiError) -> ErrorDto {
    ErrorDto {
        code: e.code.into(),
        message: e.message,
    }
}

fn unauthorized() -> ErrorDto {
    ErrorDto {
        code: "unauthorized".into(),
        message: "sign in first".into(),
    }
}

impl WebBackend {
    async fn actor(&self, token: &str) -> BResult<Actor> {
        self.0
            .auth
            .session(token)
            .await
            .map_err(|e| err(e.into()))?
            .ok_or_else(unauthorized)
    }

    async fn session(&self, token: String) -> BResult<SessionDto> {
        let a = self.actor(&token).await?;
        let user = self
            .0
            .app
            .user(a.user_id)
            .await
            .map_err(|e| err(e.into()))?;
        Ok(SessionDto {
            token,
            user: api::dto_user(&user),
        })
    }
}

#[async_trait]
impl Backend for WebBackend {
    fn session_cookie(&self) -> String {
        self.0.naming.session_cookie.clone()
    }

    fn secure_cookies(&self) -> bool {
        self.0.secure_cookies
    }

    fn session_days(&self) -> u32 {
        self.0.auth.settings().session_days
    }

    async fn me(&self, token: &str) -> Option<UserDto> {
        let a = self.actor(token).await.ok()?;
        self.0
            .app
            .user(a.user_id)
            .await
            .ok()
            .map(|u| api::dto_user(&u))
    }

    async fn sign_in(&self, b: SignInDto) -> BResult<SessionDto> {
        let t = self
            .0
            .auth
            .sign_in(&b.username, &b.password, b.totp.as_deref())
            .await
            .map_err(|e| err(e.into()))?;
        self.session(t.0).await
    }

    async fn sign_up(&self, b: SignUpDto) -> BResult<SessionDto> {
        let (_, t) = self
            .0
            .auth
            .sign_up(auth::SignUp {
                username: b.username,
                display_name: b.display_name,
                email: b.email,
                password: b.password,
                invite: b.invite,
            })
            .await
            .map_err(|e| err(e.into()))?;
        self.session(t.0).await
    }

    async fn sign_out(&self, token: &str) {
        let _ = self.0.auth.sign_out(token).await;
    }

    async fn library(&self, token: &str) -> BResult<Vec<LibraryItemDto>> {
        let a = self.actor(token).await?;
        ops::library(&self.0, a).await.map_err(err)
    }

    async fn item(&self, token: &str, id: &str) -> BResult<ItemPageDto> {
        let a = self.actor(token).await?;
        ops::item_page(&self.0, a, id).await.map_err(err)
    }

    async fn search(
        &self,
        token: &str,
        q: &str,
        kind: Option<String>,
    ) -> BResult<SearchResponseDto> {
        let a = self.actor(token).await?;
        ops::search(&self.0, a, q, kind).await.map_err(err)
    }

    async fn add(&self, token: &str, item_id: &str) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        ops::add(&self.0, a, item_id, None).await.map_err(err)
    }

    async fn log_progress(
        &self,
        token: &str,
        item_id: &str,
        p: NewProgressDto,
    ) -> BResult<ProgressEventDto> {
        let a = self.actor(token).await?;
        ops::log_progress(&self.0, a, item_id, p).await.map_err(err)
    }

    async fn rate(
        &self,
        token: &str,
        item_id: &str,
        rating: Option<u8>,
    ) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        ops::rate(&self.0, a, item_id, rating).await.map_err(err)
    }

    async fn companions(&self, token: &str) -> BResult<Vec<CompanionDto>> {
        let a = self.actor(token).await?;
        ops::companions(&self.0, a).await.map_err(err)
    }

    async fn create_companion(&self, token: &str, name: &str) -> BResult<CompanionDto> {
        let a = self.actor(token).await?;
        ops::create_companion(&self.0, a, name).await.map_err(err)
    }

    async fn lists(&self, token: &str) -> BResult<Vec<ListDto>> {
        let a = self.actor(token).await?;
        ops::lists(&self.0, a).await.map_err(err)
    }

    async fn create_list(&self, token: &str, name: &str) -> BResult<ListDto> {
        let a = self.actor(token).await?;
        ops::create_list(&self.0, a, name).await.map_err(err)
    }

    async fn add_to_list(&self, token: &str, list_id: &str, item_id: &str) -> BResult<ListDto> {
        let a = self.actor(token).await?;
        ops::add_to_list(&self.0, a, list_id, item_id)
            .await
            .map_err(err)
    }

    async fn sources(&self) -> Vec<AttributionDto> {
        ops::sources(&self.0)
    }
}
