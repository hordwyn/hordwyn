use chrono::NaiveDate;
use domain::kind::{KindGroup, ProgressModel, RelationType};

use super::attributes;
use crate::{KindModule, relations, strings};

/// Every kind in this module is in this group.
const GROUP: KindGroup = KindGroup::Read;

attributes!(
    /// Attributes of a book.
    BookAttributes {
        authors: Vec<String>,
        isbn13: Option<String>,
        page_count: Option<u32>,
        publisher: Option<String>,
        published: Option<NaiveDate>,
        language: Option<String>,
        subjects: Vec<String>,
        series: Option<String>,
        series_position: Option<f32>,
    }
);

pub struct Book;

impl KindModule for Book {
    type Attributes = BookAttributes;
    const ID: &'static str = "book";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Paged;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["authors", "subjects", "language"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["sequel", "prequel", "part_of_series", "adaptation"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["openlibrary", "hardcover"])
    }
}

attributes!(
    /// Attributes of a manga.
    MangaAttributes {
        authors: Vec<String>,
        genres: Vec<String>,
        chapter_count: Option<u32>,
        volume_count: Option<u32>,
        /// `releasing`, `finished`, `hiatus`, ...
        publishing_status: Option<String>,
        start_date: Option<NaiveDate>,
    }
);

pub struct Manga;

impl KindModule for Manga {
    type Attributes = MangaAttributes;
    const ID: &'static str = "manga";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Paged;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["genres", "authors", "publishing_status"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["sequel", "prequel", "adaptation", "side_story"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["anilist"])
    }
}

attributes!(
    /// Attributes of a comic issue or collection.
    ComicAttributes {
        publisher: Option<String>,
        series: Option<String>,
        issue_number: Option<String>,
        volume: Option<u32>,
        page_count: Option<u32>,
        cover_date: Option<NaiveDate>,
        writers: Vec<String>,
        artists: Vec<String>,
    }
);

pub struct Comic;

impl KindModule for Comic {
    type Attributes = ComicAttributes;
    const ID: &'static str = "comic";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Paged;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["publisher", "series"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["part_of_series", "collected_in"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["metron"])
    }
}
