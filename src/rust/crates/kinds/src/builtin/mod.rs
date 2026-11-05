//! The built-in kinds (D-1): movie, show, anime, manga, book, audiobook, podcast, music,
//! video game, visual novel, comic, board game.

mod listen;
mod play;
mod read;
mod screen;

pub use listen::{
    Audiobook, AudiobookAttributes, Music, MusicAttributes, Podcast, PodcastAttributes,
};
pub use play::{
    BoardGame, BoardGameAttributes, VideoGame, VideoGameAttributes, VisualNovel,
    VisualNovelAttributes,
};
pub use read::{Book, BookAttributes, Comic, ComicAttributes, Manga, MangaAttributes};
pub use screen::{
    Anime, AnimeAttributes, Movie, MovieAttributes, SeasonInfo, Show, ShowAttributes,
};

use crate::Registry;

pub fn register_all(r: &mut Registry) {
    r.register::<Movie>()
        .register::<Show>()
        .register::<Anime>()
        .register::<Manga>()
        .register::<Book>()
        .register::<Audiobook>()
        .register::<Podcast>()
        .register::<Music>()
        .register::<VideoGame>()
        .register::<VisualNovel>()
        .register::<Comic>()
        .register::<BoardGame>();
}

/// Used by `skip_serializing_if` so normalised attributes omit empty fields.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
}

/// Declares an attributes struct: every field optional, unknown fields rejected.
macro_rules! attributes {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $(
                $(#[$fm])*
                #[serde(default, skip_serializing_if = "crate::builtin::IsEmpty::is_empty")]
                pub $field: $ty,
            )*
        }
    };
}
pub(crate) use attributes;
