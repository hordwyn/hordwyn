use chrono::NaiveDate;
use domain::kind::{KindGroup, ProgressModel, RelationType};

use super::attributes;
use crate::{KindModule, relations, strings};

/// Every kind in this module is in this group.
const GROUP: KindGroup = KindGroup::Listen;

attributes!(
    /// Attributes of an audiobook.
    AudiobookAttributes {
        authors: Vec<String>,
        narrators: Vec<String>,
        duration_seconds: Option<u64>,
        isbn13: Option<String>,
        asin: Option<String>,
        publisher: Option<String>,
        published: Option<NaiveDate>,
        language: Option<String>,
    }
);

pub struct Audiobook;

impl KindModule for Audiobook {
    type Attributes = AudiobookAttributes;
    const ID: &'static str = "audiobook";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Timed;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["authors", "narrators", "language"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["adaptation", "part_of_series"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["openlibrary", "hardcover"])
    }
}

attributes!(
    /// Attributes of a podcast.
    PodcastAttributes {
        author: Option<String>,
        feed_url: Option<String>,
        categories: Vec<String>,
        episode_count: Option<u32>,
        language: Option<String>,
        explicit: Option<bool>,
    }
);

pub struct Podcast;

impl KindModule for Podcast {
    type Attributes = PodcastAttributes;
    const ID: &'static str = "podcast";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Episodic;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["categories", "language"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["podcastindex"])
    }
}

attributes!(
    /// Attributes of a music release (album, EP, single).
    MusicAttributes {
        artists: Vec<String>,
        /// `album`, `ep`, `single`, `compilation`, ...
        release_type: Option<String>,
        track_count: Option<u32>,
        duration_seconds: Option<u64>,
        release_date: Option<NaiveDate>,
        genres: Vec<String>,
        label: Option<String>,
    }
);

pub struct Music;

impl KindModule for Music {
    type Attributes = MusicAttributes;
    const ID: &'static str = "music";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Timed;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["artists", "release_type", "genres"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["remaster_of", "part_of_series"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["musicbrainz"])
    }
}
