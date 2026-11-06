//! The server side of the web app's server functions. `web` only knows this trait and the
//! `contracts` DTOs; the server binary implements it on top of the same operations as the
//! REST API, so both call the same `app` use-cases (D-3).

use async_trait::async_trait;
use contracts::{
    AttributionDto, CompanionDto, ErrorDto, ItemPageDto, LibraryEntryDto, LibraryItemDto, ListDto,
    NewProgressDto, ProgressEventDto, SearchResponseDto, SessionDto, SignInDto, SignUpDto, UserDto,
};

pub type BResult<T> = Result<T, ErrorDto>;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Name of the session cookie (from the brand).
    fn session_cookie(&self) -> String;
    fn secure_cookies(&self) -> bool;
    fn session_days(&self) -> u32;

    async fn me(&self, token: &str) -> Option<UserDto>;
    async fn sign_in(&self, b: SignInDto) -> BResult<SessionDto>;
    async fn sign_up(&self, b: SignUpDto) -> BResult<SessionDto>;
    async fn sign_out(&self, token: &str);

    async fn library(&self, token: &str) -> BResult<Vec<LibraryItemDto>>;
    async fn item(&self, token: &str, id: &str) -> BResult<ItemPageDto>;
    async fn search(
        &self,
        token: &str,
        q: &str,
        kind: Option<String>,
    ) -> BResult<SearchResponseDto>;
    async fn add(&self, token: &str, item_id: &str) -> BResult<LibraryEntryDto>;
    async fn log_progress(
        &self,
        token: &str,
        item_id: &str,
        p: NewProgressDto,
    ) -> BResult<ProgressEventDto>;
    async fn rate(
        &self,
        token: &str,
        item_id: &str,
        rating: Option<u8>,
    ) -> BResult<LibraryEntryDto>;

    async fn companions(&self, token: &str) -> BResult<Vec<CompanionDto>>;
    async fn create_companion(&self, token: &str, name: &str) -> BResult<CompanionDto>;

    async fn lists(&self, token: &str) -> BResult<Vec<ListDto>>;
    async fn create_list(&self, token: &str, name: &str) -> BResult<ListDto>;
    async fn add_to_list(&self, token: &str, list_id: &str, item_id: &str) -> BResult<ListDto>;

    async fn sources(&self) -> Vec<AttributionDto>;
}
