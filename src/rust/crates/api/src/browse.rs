//! UX phase 1 operations (REST and server functions): kinds and groups, library counts and
//! queries, tags, notes, undo, the activity feed, statistics and Home.

use app::ports::{LibrarySort, SearchQuery};
use app::{
    ActivityPage, ActivityQuery, ActivityRow, ActivityType, Amounts, Bucket, LibraryQuery,
    LibraryRow, NextStep, Totals, UpNextReason,
};
use contracts::{
    ActivityPageDto, ActivityPersonDto, ActivityQueryDto, ActivityRowDto, AmountsDto, CountCellDto,
    CountDto, EpisodeRefDto, HomeDto, KindDto, KindGroupDto, KindTotalsDto, KindsDto,
    LibraryCountsDto, LibraryEntryDto, LibraryQueryDto, LibraryRowDto, LibraryRowsDto, NextStepDto,
    ProgressEventDto, ProgressSummaryDto, SeriesPointDto, StatsSeriesDto, StatsTotalsDto, TagDto,
    TotalsDto, UpNextDto,
};
use domain::Timestamp;
use domain::ids::{CompanionId, EventId, ItemId};
use domain::kind::{KindGroup, KindId, Status};
use domain::user::Actor;

use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiError, ApiState, dto};

/// Text-filter matches considered by a library query (the search port's largest page).
pub const TEXT_FILTER_MATCHES: usize = 100;

/// A group id; `other` is valid, unknown ids are not.
///
/// # Errors
/// `bad_request` for an unknown group.
pub fn parse_group(s: &str) -> ApiResult<KindGroup> {
    KindGroup::ALL
        .into_iter()
        .find(|g| g.as_str() == s)
        .ok_or_else(|| ApiError::bad_request(format!("unknown group `{s}`")))
}

fn parse_kind(s: &str) -> ApiResult<KindId> {
    KindId::new(s).map_err(|e| ApiError::bad_request(e.to_string()))
}

/// A library paging cursor (`after`, DEC-416).
///
/// # Errors
/// Text that is no cursor of ours (`bad_request`).
pub fn parse_cursor(s: &str) -> ApiResult<app::ports::LibraryCursor> {
    app::ports::LibraryCursor::decode(s)
        .ok_or_else(|| ApiError::bad_request("`after` is not a cursor from a previous page"))
}

fn parse_sort(s: &str) -> ApiResult<LibrarySort> {
    Ok(match s {
        "last_activity" => LibrarySort::LastActivity,
        "title" => LibrarySort::Title,
        "added" => LibrarySort::Added,
        "year" => LibrarySort::Year,
        "rating" => LibrarySort::Rating,
        "plays" => LibrarySort::Plays,
        _ => return Err(ApiError::bad_request(format!("unknown sort `{s}`"))),
    })
}

/// Kinds and groups.
#[must_use]
pub fn kinds(s: &ApiState) -> KindsDto {
    let all = s.app.kinds.all();
    let groups = KindGroup::ALL
        .into_iter()
        .filter_map(|g| {
            let kinds: Vec<String> = all
                .iter()
                .filter(|d| d.group == g)
                .map(|d| d.id.to_string())
                .collect();
            (g != KindGroup::Other || !kinds.is_empty()).then(|| KindGroupDto {
                id: g.as_str().into(),
                label_key: format!("kind-group-{}", g.as_str()),
                kinds,
            })
        })
        .collect();
    let kinds = all
        .iter()
        .map(|d| KindDto {
            id: d.id.to_string(),
            group: d.group.as_str().into(),
            label_key: format!("kind-{}", d.id),
            progress_model: d.progress_model.as_str().into(),
            statuses: d.statuses().map(String::from).collect(),
            facets: d.facets.clone(),
        })
        .collect();
    KindsDto { groups, kinds }
}

/// # Errors
/// Storage errors.
pub async fn library_counts(s: &ApiState, a: Actor) -> ApiResult<LibraryCountsDto> {
    let c = s.app.library_counts(a).await?;
    let count = |key: &str, count: u64| CountDto {
        key: key.into(),
        count,
    };
    Ok(LibraryCountsDto {
        total: c.total,
        groups: c
            .groups
            .iter()
            .map(|(g, n)| count(g.as_str(), *n))
            .collect(),
        kinds: c.kinds.iter().map(|(k, n)| count(k.as_str(), *n)).collect(),
        statuses: c
            .statuses
            .iter()
            .map(|(st, n)| count(st.as_str(), *n))
            .collect(),
        cells: c
            .cells
            .into_iter()
            .map(|x| CountCellDto {
                kind: x.kind.to_string(),
                group: x.group.as_str().into(),
                status: x.status.as_str().into(),
                count: x.count,
            })
            .collect(),
    })
}

/// Items of the viewer's library matching `text` (library search index); 503
/// `search_unavailable` when the index cannot answer, so the page offers Retry instead of
/// "nothing matches" (UX1-10).
async fn text_matches(s: &ApiState, a: Actor, text: &str) -> ApiResult<Vec<ItemId>> {
    let hits = s
        .search
        .search(SearchQuery {
            index: "library".into(),
            text: text.to_string(),
            viewer: a.user_id,
            filters: vec![],
            limit: TEXT_FILTER_MATCHES,
        })
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "library text filter failed");
            ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "search_unavailable",
                "search is not available",
            )
        })?;
    Ok(hits
        .iter()
        .filter_map(|h| h.fields.get("item_id")?.as_str())
        .filter_map(|id| uuid::Uuid::parse_str(id).ok())
        .map(ItemId)
        .collect())
}

/// The Library page's query.
///
/// # Errors
/// Unknown group, kind or sort (`bad_request`); a facet the kinds do not have (`invalid`).
pub async fn library_query(
    s: &ApiState,
    a: Actor,
    q: LibraryQueryDto,
) -> ApiResult<LibraryRowsDto> {
    let items = match q.q.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(text) => Some(text_matches(s, a, text).await?),
        None => None,
    };
    let query = LibraryQuery {
        group: q.group.as_deref().map(parse_group).transpose()?,
        kinds: q
            .kinds
            .iter()
            .map(|k| parse_kind(k))
            .collect::<ApiResult<_>>()?,
        statuses: q.statuses.iter().map(Status::new).collect(),
        tags: q.tags,
        facets: q
            .facets
            .into_iter()
            .map(|f| (f.attribute, f.value))
            .collect(),
        items,
        sort: q
            .sort
            .as_deref()
            .map(parse_sort)
            .transpose()?
            .unwrap_or_default(),
        descending: q.descending,
        after: q.after.as_deref().map(parse_cursor).transpose()?,
        limit: q.limit,
    };
    let limit = if query.limit == 0 {
        app::DEFAULT_PAGE
    } else {
        query.limit.clamp(1, app::App::MAX_PAGE)
    };
    let page = s.app.library_query(a, query).await?;
    let mut rows = Vec::with_capacity(page.rows.len());
    for r in &page.rows {
        rows.push(row(s, r).await);
    }
    Ok(LibraryRowsDto {
        rows,
        total: page.total,
        next: page.next.as_ref().map(app::ports::LibraryCursor::encode),
        limit,
    })
}

/// A next step with its label key.
#[must_use]
pub fn next_step(step: &NextStep, group: KindGroup, kind: &KindId) -> NextStepDto {
    let watch = group == KindGroup::Watch;
    let (action, label) = match step {
        NextStep::Finish { again: false } if watch => ("finish", "next-step-watched"),
        NextStep::Finish { again: true } if watch => ("finish", "next-step-watched-again"),
        NextStep::Finish { again: false } => ("finish", "next-step-finished"),
        NextStep::Finish { again: true } => ("finish", "next-step-finished-again"),
        NextStep::Episode { .. } if group == KindGroup::Listen => {
            ("episode", "next-step-episode-listened")
        }
        NextStep::Episode { .. } => ("episode", "next-step-episode"),
        NextStep::CaughtUp => ("caught_up", "next-step-caught-up"),
        NextStep::Page { .. } => ("page", "next-step-update-page"),
        NextStep::Position { .. } => ("position", "next-step-update-position"),
        NextStep::Session { .. } if kind.as_str() == "board_game" => {
            ("session", "next-step-log-play")
        }
        NextStep::Session { .. } => ("session", "next-step-log-session"),
    };
    let mut d = NextStepDto {
        action: action.into(),
        label_key: label.into(),
        ..NextStepDto::default()
    };
    match *step {
        NextStep::Finish { again } => d.again = again,
        NextStep::Episode { season, episode } => {
            d.season = Some(season);
            d.episode = Some(episode);
        }
        NextStep::CaughtUp => {}
        NextStep::Page {
            page,
            chapter,
            pages,
            chapters,
        } => {
            d.page = page;
            d.chapter = chapter;
            d.pages = pages;
            d.chapters = chapters;
        }
        NextStep::Position { seconds, total } => {
            d.seconds = seconds;
            d.total_seconds = total;
        }
        NextStep::Session { plays } => d.plays = Some(plays),
    }
    d
}

fn summary(r: &LibraryRow) -> ProgressSummaryDto {
    let s = &r.summary;
    ProgressSummaryDto {
        events: s.events,
        finished: s.finished,
        episodes_watched: u32::try_from(
            s.episodes.iter().filter(|(season, _)| *season > 0).count(),
        )
        .unwrap_or(u32::MAX),
        episode_views: s.episode_views,
        episodes_total: r.episodes_total(),
        last_episode: s
            .last_episode()
            .map(|(season, episode)| EpisodeRefDto { season, episode }),
        page: s.page,
        chapter: s.chapter,
        volume: s.volume,
        pages_read: s.pages_read,
        position_seconds: s.position,
        seconds_listened: s.seconds_listened,
        sessions: s.sessions,
        first_at: s.first_at.map(|t| t.to_rfc3339()),
        last_at: s.last_at.map(|t| t.to_rfc3339()),
        fraction: r.fraction(),
        rewatch: r.rewatch.as_ref().map(|w| contracts::browse::RewatchDto {
            start: w.start.0.to_string(),
            done: w.done,
            running: w.running,
        }),
    }
}

/// A library row DTO.
pub async fn row(s: &ApiState, r: &LibraryRow) -> LibraryRowDto {
    LibraryRowDto {
        entry: dto::entry(&r.entry),
        item: dto::item(s, &r.item).await,
        group: r.group.as_str().into(),
        last_activity: r.last_activity.to_rfc3339(),
        tags: r.tags.clone(),
        summary: summary(r),
        next_step: next_step(&r.next_step, r.group, &r.item.kind),
    }
}

/// Summary, next step and tags of a library row (item page).
#[must_use]
pub fn item_state(r: app::LibraryRow) -> (ProgressSummaryDto, NextStepDto, Vec<String>) {
    (
        summary(&r),
        next_step(&r.next_step, r.group, &r.item.kind),
        r.tags,
    )
}

// ------------------------------------------------------------ tags, note, undo

/// # Errors
/// Storage errors.
pub async fn tags(s: &ApiState, a: Actor) -> ApiResult<Vec<TagDto>> {
    Ok(s.app
        .tags(a)
        .await?
        .into_iter()
        .map(|t| TagDto {
            tag: t.tag,
            entries: t.entries,
        })
        .collect())
}

/// # Errors
/// Invalid tag or id, unknown item.
pub async fn add_tag(s: &ApiState, a: Actor, item_id: &str, tag: &str) -> ApiResult<Vec<String>> {
    Ok(s.app.add_tag(a, parse_id(item_id, ItemId)?, tag).await?)
}

/// # Errors
/// Not in the library.
pub async fn remove_tag(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    tag: &str,
) -> ApiResult<Vec<String>> {
    Ok(s.app.remove_tag(a, parse_id(item_id, ItemId)?, tag).await?)
}

/// # Errors
/// Invalid new name.
pub async fn rename_tag(s: &ApiState, a: Actor, from: &str, to: &str) -> ApiResult<u64> {
    Ok(s.app.rename_tag(a, from, to).await?)
}

/// Set or clear the private note of an entry (LWW like sync's `set_notes`).
///
/// # Errors
/// Unknown item; a note longer than 20 000 characters.
pub async fn set_note(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    note: Option<String>,
) -> ApiResult<LibraryEntryDto> {
    let note = note.filter(|n| !n.trim().is_empty());
    if note
        .as_ref()
        .is_some_and(|n| n.chars().count() > domain::review::MAX_REVIEW_CHARS)
    {
        return Err(ApiError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "invalid",
            "note is too long",
        ));
    }
    let e = s
        .app
        .set_notes(a, parse_id(item_id, ItemId)?, note, None)
        .await?;
    Ok(dto::entry(&e))
}

/// # Errors
/// Someone else's or unknown event (`not_found`), too old or an undo (`conflict`).
pub async fn undo(s: &ApiState, a: Actor, event_id: &str) -> ApiResult<ProgressEventDto> {
    let e = s.app.undo_progress(a, parse_id(event_id, EventId)?).await?;
    Ok(dto::progress(&e))
}

// ------------------------------------------------------------ activity

/// `<micros>_<uuid>`: an opaque paging key.
#[must_use]
pub fn encode_cursor(key: (Timestamp, uuid::Uuid)) -> String {
    format!("{}_{}", key.0.timestamp_micros(), key.1)
}

/// # Errors
/// `bad_request` for a malformed cursor.
pub fn decode_cursor(s: &str) -> ApiResult<(Timestamp, uuid::Uuid)> {
    let bad = || ApiError::bad_request("invalid cursor");
    let (micros, id) = s.split_once('_').ok_or_else(bad)?;
    let t = chrono::DateTime::from_timestamp_micros(micros.parse().map_err(|_| bad())?)
        .ok_or_else(bad)?;
    Ok((t, uuid::Uuid::parse_str(id).map_err(|_| bad())?))
}

fn activity_person(p: &app::ActivityPerson) -> ActivityPersonDto {
    ActivityPersonDto {
        companion_id: p.companion_id.0.to_string(),
        name: p.name.clone(),
        is_me: p.is_me,
        score: p.score,
        winner: p.winner,
    }
}

fn activity_play(p: app::ActivityPlay) -> contracts::ActivityPlayDto {
    contracts::ActivityPlayDto {
        duration_minutes: p.duration_minutes,
        coop: p.coop.map(|c| c.as_str().to_string()),
    }
}

/// Plays counted, with the caller's results.
#[must_use]
pub fn play_totals(t: app::PlayTotals) -> contracts::PlayTotalsDto {
    contracts::PlayTotalsDto {
        plays: t.plays,
        minutes: t.minutes,
        decided: t.decided,
        wins: t.wins,
    }
}

/// A play of the item page's History tab.
#[must_use]
pub fn item_play(p: &app::ItemPlay) -> contracts::ItemPlayDto {
    contracts::ItemPlayDto {
        session_id: p.session_id.0.to_string(),
        play: activity_play(p.play),
        participants: p.participants.iter().map(activity_person).collect(),
    }
}

fn activity_row(r: &ActivityRow, zone: app::time_zone::Tz) -> ActivityRowDto {
    ActivityRowDto {
        at: r.at.to_rfc3339(),
        local_day: app::time_zone::local_day(r.at, zone),
        local_time: app::time_zone::local_time(r.at, zone),
        row_type: match r.row_type {
            ActivityType::Progress => "progress".into(),
            ActivityType::Session => "session".into(),
        },
        item_id: r.item_id.0.to_string(),
        title: r.title.clone(),
        kind: r.kind.to_string(),
        group: r.group.as_str().into(),
        progress: r
            .progress
            .as_ref()
            .and_then(|p| serde_json::to_value(p).ok()),
        event_id: r.event_id.map(|e| e.0.to_string()),
        session_id: r.session_id.map(|x| x.0.to_string()),
        participants: r.participants.iter().map(activity_person).collect(),
        play: r.play.map(activity_play),
        shared: r.shared,
        owner_name: r.owner_name.clone(),
        copied: r.copied,
        undoable: r.undoable,
    }
}

fn activity_page(p: &ActivityPage, zone: app::time_zone::Tz) -> ActivityPageDto {
    ActivityPageDto {
        rows: p.rows.iter().map(|r| activity_row(r, zone)).collect(),
        next: p.next.map(encode_cursor),
    }
}

/// # Errors
/// Malformed filters (`bad_request`).
pub async fn activity(s: &ApiState, a: Actor, q: ActivityQueryDto) -> ApiResult<ActivityPageDto> {
    let types = match q.row_type.as_deref() {
        None | Some("") => vec![],
        Some("progress") => vec![ActivityType::Progress],
        Some("session") => vec![ActivityType::Session],
        Some(other) => return Err(ApiError::bad_request(format!("unknown row type `{other}`"))),
    };
    let query = ActivityQuery {
        before: q.before.as_deref().map(decode_cursor).transpose()?,
        limit: q.limit.unwrap_or(0),
        group: q.group.as_deref().map(parse_group).transpose()?,
        kind: q.kind.as_deref().map(parse_kind).transpose()?,
        person: q
            .person
            .as_deref()
            .map(|p| parse_id(p, CompanionId))
            .transpose()?,
        types,
        shared_only: q.shared,
        year: q.year,
    };
    let zone = s.app.user_zone(a.user_id).await?;
    Ok(activity_page(&s.app.activity_feed(a, query).await?, zone))
}

// ------------------------------------------------------------ stats and home

pub(crate) fn amounts(a: &Amounts) -> AmountsDto {
    AmountsDto {
        events: a.events,
        completions: a.completions,
        finished: a.finished,
        episodes: a.episodes,
        pages: a.pages,
        plays: a.plays,
        minutes: a.minutes,
    }
}

fn totals(group: &str, t: &Totals) -> TotalsDto {
    TotalsDto {
        group: group.into(),
        entries: t.entries,
        planned: t.planned,
        in_progress: t.in_progress,
        completed: t.completed,
        paused: t.paused,
        dropped: t.dropped,
        amounts: amounts(&t.amounts),
    }
}

/// # Errors
/// Storage errors.
pub async fn stats_totals(s: &ApiState, a: Actor) -> ApiResult<StatsTotalsDto> {
    let t = s.app.stats_totals(a).await?;
    Ok(StatsTotalsDto {
        overall: totals("all", &t.overall),
        groups: t
            .groups
            .iter()
            .map(|g| totals(g.group.as_str(), &g.totals))
            .collect(),
        kinds: t
            .kinds
            .iter()
            .map(|k| KindTotalsDto {
                kind: k.kind.as_str().to_string(),
                amounts: amounts(&k.amounts),
            })
            .collect(),
        undated_completions: t.undated_completions,
    })
}

/// Activity per `month` (default) or `year` for a group (`all` or absent: every group).
///
/// # Errors
/// Unknown group or bucket.
pub async fn stats_series(
    s: &ApiState,
    a: Actor,
    group: Option<String>,
    bucket: Option<String>,
) -> ApiResult<StatsSeriesDto> {
    let g = match group.as_deref() {
        None | Some("all") => None,
        Some(x) => Some(parse_group(x)?),
    };
    let b = match bucket.as_deref() {
        None | Some("month") => Bucket::Month,
        Some("year") => Bucket::Year,
        Some(x) => return Err(ApiError::bad_request(format!("unknown bucket `{x}`"))),
    };
    let points = s.app.stats_series(a, g, b).await?;
    Ok(StatsSeriesDto {
        group: g.map_or("all", KindGroup::as_str).into(),
        bucket: match b {
            Bucket::Month => "month".into(),
            Bucket::Year => "year".into(),
        },
        points: points
            .iter()
            .map(|p| SeriesPointDto {
                bucket: p.bucket.clone(),
                amounts: amounts(&p.amounts),
            })
            .collect(),
    })
}

/// Rows per Home section (design.md 6.1, OD-03).
pub const HOME_CONTINUE: u32 = 6;
pub const HOME_UP_NEXT: u32 = 6;
pub const HOME_ACTIVITY: u32 = 5;

/// Everything the signed-in Home page shows. The five parts are independent, so they are
/// read concurrently (WP4-1: Home is their slowest part, not their sum).
///
/// # Errors
/// Storage errors.
pub async fn home(s: &ApiState, a: Actor) -> ApiResult<HomeDto> {
    let counts = async { Ok::<_, ApiError>(s.app.library_counts(a).await?) };
    let continue_rows = async {
        let mut out = Vec::new();
        for r in s.app.continue_rows(a, HOME_CONTINUE).await? {
            out.push(row(s, &r).await);
        }
        Ok::<_, ApiError>(out)
    };
    let up_next = async {
        let mut out = Vec::new();
        for u in s.app.up_next(a, HOME_UP_NEXT).await? {
            out.push(UpNextDto {
                row: row(s, &u.row).await,
                reason: match u.reason {
                    UpNextReason::NextInSeries => "next_in_series".into(),
                    UpNextReason::Planned => "planned".into(),
                },
            });
        }
        Ok::<_, ApiError>(out)
    };
    let activity = async {
        let page = s
            .app
            .activity_feed(
                a,
                ActivityQuery {
                    limit: HOME_ACTIVITY,
                    ..ActivityQuery::default()
                },
            )
            .await?;
        Ok::<_, ApiError>(activity_page(&page, s.app.user_zone(a.user_id).await?))
    };
    let (counts, continue_rows, up_next, activity, stats) =
        tokio::try_join!(counts, continue_rows, up_next, activity, stats_totals(s, a))?;
    Ok(HomeDto {
        library_total: counts.total,
        continue_rows,
        up_next,
        activity,
        stats,
    })
}
