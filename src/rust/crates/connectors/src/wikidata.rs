//! Wikidata (board games). Terms, limits, attribution and the class list:
//! `docs/connectors/wikidata.md`.
//!
//! The Action API on `www.wikidata.org` only (no SPARQL, no dumps, no Commons, no writes).
//! No key; Wikimedia's User-Agent policy asks for a contact, so the governed client sends
//! the operator contact ([`ContactUse::Required`]) and the source is not asked without one.
//! A search is two requests in series: the full-text search limited to items that are an
//! instance of one of [`CLASSES`], then one `wbgetentities` for all hits. The details are
//! the entity and one more `wbgetentities` for the labels of its genres and mechanics.
//! No image is read (DEC-642): P18 points to Commons. Written from the public API
//! documentation and recorded answers alone (DEC-169); the software behind the API is GPL
//! and its code is not read.

use app::ports::{HttpError, Lane};
use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, ContactUse, Ctx,
    Details, DetailsRequest, RateLimit, Search, SearchRequest, Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::BoardGameAttributes;
use serde_json::{Map, Value};

use crate::util::get_json_with_headers;

pub const ID: &str = "wikidata";
const DEFAULT_BASE: &str = "https://www.wikidata.org";
const KIND: &str = "board_game";

/// The classes a search hit has to be an instance of (P31): Q131436 (board game), Q60474521
/// (board game expansion), the subclasses of board game with games filed under them, and
/// the tabletop and card game classes board games are also filed under, and the sibling
/// classes found by measuring what the filter loses (98.2 % of the items with a
/// BoardGameGeek id are kept, 2026-11-13). The list, the counts, what was left out and how
/// to measure it again are in `docs/connectors/wikidata.md`. A constant,
/// because resolving the class tree per search would need SPARQL. Regenerate it with a
/// release.
pub const CLASSES: &[&str] = &[
    "Q131436",    // board game
    "Q60474521",  // board game expansion
    "Q3244175",   // tabletop game
    "Q142714",    // card game
    "Q3177859",   // dedicated deck card game
    "Q15804899",  // deck-building game
    "Q1191150",   // trick-taking game
    "Q1150710",   // strategy game
    "Q1272194",   // tile-based game
    "Q4931403",   // board wargame
    "Q1783817",   // cooperative board game
    "Q985834",    // shogi variant
    "Q267989",    // mancala
    "Q18703581",  // game on cell board
    "Q5188437",   // cross and circle game
    "Q2796140",   // variants of draughts
    "Q1275314",   // go variant
    "Q21608615",  // economic simulation board game
    "Q788553",    // German-style board game
    "Q1182245",   // deduction game
    "Q10924689",  // four-in-a-row
    "Q10499647",  // quiz board game
    "Q17144589",  // train game
    "Q1099522",   // race game
    "Q1548611",   // Game of the Goose
    "Q1637382",   // Hounds and Jackals
    "Q31977010",  // Gonu
    "Q4927217",   // block wargame
    "Q48997832",  // legacy game
    "Q66152389",  // historical board game
    "Q104714331", // bluffing game
    // sibling classes of board game that whole families of common games are filed under
    // (review WD-1, measured 2026-11-13; LB-34)
    "Q1515156",   // dice game
    "Q573573",    // abstract strategy game
    "Q299191",    // chess variant
    "Q1509934",   // children's game
    "Q14947863",  // parlour game
    "Q839864",    // party game
    "Q5161688",   // connection game
    "Q133889197", // xiangqi variant
];

/// Hits of one search (the source allows up to 50 ids per `wbgetentities`).
const SEARCH_LIMIT: u32 = 20;
/// Linked items whose labels are asked with one request.
const LABEL_BATCH: usize = 50;
/// Characters of typed text sent with a search.
const MAX_QUERY_CHARS: usize = 200;
/// GTINs kept of one item.
const MAX_GTINS: usize = 5;
/// Seconds a successful answer may be kept (the API's `maxage` / `smaxage`; the source
/// never marks an error answer cacheable).
const MAX_AGE: &str = "86400";
/// `maxlag` of calls nobody waits for (API:Etiquette).
const MAXLAG: &str = "5";
/// Wait after a `maxlag` or `ratelimited` error answer that names no other.
const LAG_WAIT_MS: u64 = 5_000;
/// Languages asked after the request's, `en` and `mul`: the API answers only the languages
/// asked, so an item without a label in those still has one to show (WD-2, LB-35).
const MORE_LANGUAGES: &[&str] = &[
    "de", "fr", "es", "it", "nl", "pl", "pt", "sv", "da", "nb", "fi", "cs", "hu", "ru", "uk", "ja",
    "zh", "ko",
];

const MINUTE: &str = "Q7727";
const HOUR: &str = "Q25235";
const YEARS_OLD: &str = "Q24564698";
const YEAR: &str = "Q577";

pub struct Wikidata {
    info: ConnectorInfo,
    base: String,
}

impl Default for Wikidata {
    fn default() -> Self {
        Self::new()
    }
}

fn host_of(base: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(u.port().map_or(host.clone(), |p| format!("{host}:{p}")))
        })
        .unwrap_or_default()
}

impl Wikidata {
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE)
    }

    /// Another host for the API (tests, a mirror); `/w/api.php` is appended.
    #[must_use]
    pub fn with_base_url(base: &str) -> Self {
        let base = base.trim_end_matches('/').to_string();
        Self {
            info: ConnectorInfo {
                id: ID.into(),
                name: "Wikidata".into(),
                kinds: vec![KIND.into()],
                capabilities: vec![Capability::Search, Capability::Details],
                // API:Etiquette asks for serial requests; 120 a minute at most, under the
                // 200 Wikimedia allows a client with a compliant User-Agent
                rate_limit: RateLimit {
                    per_second: 2,
                    burst: 2,
                    max_concurrency: 1,
                },
                auth: Auth::None,
                attribution: Attribution {
                    text: "Data from Wikidata (CC0).".into(),
                    url: "https://www.wikidata.org/".into(),
                    licence: "CC0 1.0 (structured data); the credit is a courtesy".into(),
                },
                hosts: vec![host_of(&base)],
                cache_ttl_secs: 24 * 3600,
                contact: ContactUse::Required,
            },
            base,
        }
    }

    fn api(&self, mut query: Vec<(&str, &str)>, lane: Lane) -> Result<String, ConnectorError> {
        let mut u = url::Url::parse(&format!("{}/w/api.php", self.base))
            .map_err(|e| ConnectorError::Parse(format!("base url: {e}")))?;
        query.extend([("maxage", MAX_AGE), ("smaxage", MAX_AGE)]);
        // a person waits for an interactive call: it is never refused for replication lag
        if lane != Lane::Interactive {
            query.push(("maxlag", MAXLAG));
        }
        u.query_pairs_mut().extend_pairs(query);
        Ok(u.to_string())
    }

    /// The `wbgetentities` request for `ids` in `language` (then `en`, then `mul`): with
    /// labels, descriptions, claims and sitelinks when `full`, else the labels only.
    ///
    /// # Errors
    /// A base URL that does not parse.
    pub fn entities_url(
        &self,
        ids: &[String],
        full: bool,
        language: &str,
        lane: Lane,
    ) -> Result<String, ConnectorError> {
        let ids = ids.join("|");
        let languages = languages(language).join("|");
        let props = if full {
            "labels|descriptions|claims|sitelinks"
        } else {
            "labels"
        };
        self.api(
            vec![
                ("action", "wbgetentities"),
                ("ids", &ids),
                ("props", props),
                ("languages", &languages),
                ("format", "json"),
            ],
            lane,
        )
    }

    /// The class-filtered full-text search for `typed` (at most [`SEARCH_LIMIT`] hits).
    ///
    /// # Errors
    /// A base URL that does not parse.
    pub fn search_url(
        &self,
        typed: &str,
        limit: u32,
        lane: Lane,
    ) -> Result<String, ConnectorError> {
        let limit = limit.clamp(1, SEARCH_LIMIT).to_string();
        let filter = CLASSES
            .iter()
            .map(|c| format!("P31={c}"))
            .collect::<Vec<_>>()
            .join("|");
        let search = format!("{} haswbstatement:{filter}", plain_words(typed));
        self.api(
            vec![
                ("action", "query"),
                ("list", "search"),
                ("srnamespace", "0"),
                ("srlimit", &limit),
                ("srprop", ""),
                ("srsearch", &search),
                ("format", "json"),
                ("formatversion", "2"),
            ],
            lane,
        )
    }

    async fn entities(
        &self,
        ctx: &Ctx,
        op: &str,
        ids: &[String],
        full: bool,
        language: &str,
    ) -> Result<Map<String, Value>, ConnectorError> {
        let url = self.entities_url(ids, full, language, ctx.lane)?;
        let body = get_api(ctx, op, &url).await?;
        Ok(body
            .get("entities")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default())
    }

    /// The labels of the linked items, by id; items without a label in the languages asked
    /// are absent.
    async fn labels(
        &self,
        ctx: &Ctx,
        ids: &[String],
        language: &str,
    ) -> Result<Map<String, Value>, ConnectorError> {
        if ids.is_empty() {
            return Ok(Map::new());
        }
        let entities = self.entities(ctx, "labels", ids, false, language).await?;
        Ok(entities
            .into_iter()
            .filter_map(|(id, e)| {
                let l = text_in(e.get("labels"), &languages(language))?.to_string();
                Some((id, Value::String(l)))
            })
            .collect())
    }
}

fn headers() -> Vec<(String, String)> {
    vec![("Accept".into(), "application/json".into())]
}

/// The primary subtag of a language tag (`de` of `de-DE`), when it is two or three
/// letters; else `en`.
fn language_of(tag: Option<&str>) -> String {
    tag.and_then(|t| t.split(['-', '_']).next())
        .map(str::to_ascii_lowercase)
        .filter(|l| (2..=3).contains(&l.len()) && l.chars().all(|c| c.is_ascii_lowercase()))
        .unwrap_or_else(|| "en".into())
}

/// The languages asked: the request's, then `en`, then `mul` (labels valid in every
/// language), then [`MORE_LANGUAGES`]; each once.
fn languages(language: &str) -> Vec<&str> {
    let mut out = vec![language];
    for l in ["en", "mul"].iter().chain(MORE_LANGUAGES).copied() {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

/// What a person typed, as plain words for the search: whitespace collapsed, at most
/// [`MAX_QUERY_CHARS`] characters, and the source's search syntax taken out (a `:` ends a
/// keyword such as `haswbstatement:`, quotes make phrases, a leading `-` or `!` negates,
/// `OR`/`AND`/`NOT`, `&&` and `||` join), so the class filter stays the only condition.
fn plain_words(typed: &str) -> String {
    let cleaned: String = typed
        .chars()
        .map(|c| if matches!(c, ':' | '"') { ' ' } else { c })
        .collect();
    let mut out = String::new();
    for word in cleaned.split_whitespace() {
        let word = word.trim_start_matches(['-', '!']);
        let word = match word {
            "" | "&&" | "||" => continue,
            "OR" | "AND" | "NOT" => word.to_ascii_lowercase(),
            w => w.to_string(),
        };
        let used = out.chars().count() + usize::from(!out.is_empty());
        let room = MAX_QUERY_CHARS.saturating_sub(used);
        if room == 0 {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.extend(word.chars().take(room));
    }
    out
}

/// Is `id` an item id (`Q` and digits without a leading zero)? Anything else is never put
/// into a request.
fn is_item_id(id: &str) -> bool {
    id.strip_prefix('Q').is_some_and(|d| {
        (1..=12).contains(&d.len()) && !d.starts_with('0') && d.chars().all(|c| c.is_ascii_digit())
    })
}

/// GET an API answer and read its error object. A `maxlag` or `ratelimited` error arrives
/// with HTTP 200, so the governed client cannot tell: the connector asks it to pause the
/// source for the wait the answer names (WD-4, LB-37).
async fn get_api(ctx: &Ctx, op: &str, url: &str) -> Result<Value, ConnectorError> {
    let (body, answer_headers): (Value, _) =
        get_json_with_headers(ctx, ID, op, url, headers()).await?;
    let result = check_error(&body, &answer_headers);
    if let Err(ConnectorError::Http(HttpError::RateLimited { retry_after_ms })) = &result {
        ctx.http
            .pause(ID, std::time::Duration::from_millis(*retry_after_ms));
    }
    result.map(|()| body)
}

/// How long a `maxlag`/`ratelimited` answer asks to wait: its `Retry-After` in seconds, else
/// the `lag` it names (rounded up to whole seconds), else [`LAG_WAIT_MS`].
fn wait_ms(error: &Value, answer_headers: &[(String, String)]) -> u64 {
    let retry_after = answer_headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, v)| v.trim().parse::<u64>().ok());
    let lag = error
        .get("lag")
        .and_then(Value::as_f64)
        .filter(|l| l.is_finite() && *l > 0.0 && *l < 3600.0)
        // positive, finite and under an hour: the cast is exact
        .map(|l| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let secs = l.ceil() as u64;
            secs
        });
    retry_after
        .or(lag)
        .map_or(LAG_WAIT_MS, |s| s.saturating_mul(1000))
}

/// An error object in an answer (the API answers errors with HTTP 200): `maxlag` and
/// `ratelimited` ask to come back later, `no-such-entity` is an unknown id.
fn check_error(body: &Value, answer_headers: &[(String, String)]) -> Result<(), ConnectorError> {
    let Some(error) = body.get("error") else {
        return Ok(());
    };
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match code {
        "maxlag" | "ratelimited" => Err(ConnectorError::Http(HttpError::RateLimited {
            retry_after_ms: wait_ms(error, answer_headers),
        })),
        "no-such-entity" => Err(ConnectorError::NotFound),
        _ => {
            let info = error
                .get("info")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Err(ConnectorError::Parse(format!("api error {code}: {info}")))
        }
    }
}

/// The value of a language map (`labels`, `descriptions`) in the first of `languages` that
/// has one; an empty map may arrive as `[]`.
fn text_in<'a>(map: Option<&'a Value>, languages: &[&str]) -> Option<&'a str> {
    let map = map?.as_object()?;
    languages
        .iter()
        .filter_map(|l| map.get(*l)?.get("value")?.as_str())
        .find(|v| !v.trim().is_empty())
}

/// The label in the request's language, else `en`, else `mul`, else the first of the wider
/// set, else any.
fn label<'a>(entity: &'a Value, language: &str) -> Option<&'a str> {
    let labels = entity.get("labels");
    text_in(labels, &languages(language)).or_else(|| {
        labels?
            .as_object()?
            .values()
            .filter_map(|v| v.get("value")?.as_str())
            .find(|v| !v.trim().is_empty())
    })
}

/// The data values of the statements of `property` that have a value and are not
/// deprecated, with their rank, in their order ("unknown value" and "no value" are skipped).
fn values<'a>(entity: &'a Value, property: &str) -> Vec<(&'a str, &'a Value)> {
    let Some(statements) = entity
        .get("claims")
        .and_then(|c| c.get(property))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    statements
        .iter()
        .filter_map(|s| {
            let rank = s.get("rank").and_then(Value::as_str).unwrap_or("normal");
            let snak = s.get("mainsnak")?;
            if rank == "deprecated" || snak.get("snaktype").and_then(Value::as_str) != Some("value")
            {
                return None;
            }
            Some((rank, snak.get("datavalue")?.get("value")?))
        })
        .collect()
}

/// The values of the best rank: the preferred ones when there are any, else the normal ones.
fn best<'a>(entity: &'a Value, property: &str) -> Vec<&'a Value> {
    let all = values(entity, property);
    let preferred = all.iter().any(|(rank, _)| *rank == "preferred");
    all.into_iter()
        .filter(|(rank, _)| !preferred || *rank == "preferred")
        .map(|(_, v)| v)
        .collect()
}

/// A quantity value.
struct Quantity<'a> {
    amount: f64,
    lower: f64,
    upper: f64,
    /// The unit's item id, or `1` for none.
    unit: &'a str,
}

fn quantity(v: &Value) -> Option<Quantity<'_>> {
    let number = |k: &str| v.get(k)?.as_str()?.parse::<f64>().ok();
    let amount = number("amount")?;
    let unit = v.get("unit")?.as_str()?;
    Some(Quantity {
        amount,
        lower: number("lowerBound").unwrap_or(amount),
        upper: number("upperBound").unwrap_or(amount),
        unit: unit.rsplit('/').next().unwrap_or(unit),
    })
}

/// A positive whole number of at most `u32::MAX` (rounded).
fn whole(x: f64) -> Option<u32> {
    let r = x.round();
    // the range check makes the cast exact
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (r >= 1.0 && r <= f64::from(u32::MAX)).then_some(r as u32)
}

/// The first value of the best rank of a property in one of `units`.
fn amount_in(entity: &Value, property: &str, units: &[&str]) -> Option<u32> {
    best(entity, property)
        .into_iter()
        .filter_map(quantity)
        .find(|q| units.contains(&q.unit))
        .and_then(|q| whole(q.amount))
}

/// P2047 in minutes: the lowest lower bound and the highest upper bound of the statements
/// in minutes or hours; any other unit is skipped.
fn playtime(entity: &Value) -> (Option<u32>, Option<u32>) {
    let mut low: Option<u32> = None;
    let mut high: Option<u32> = None;
    for q in best(entity, "P2047").into_iter().filter_map(quantity) {
        let factor = match q.unit {
            MINUTE => 1.0,
            HOUR => 60.0,
            _ => continue,
        };
        if let (Some(l), Some(u)) = (whole(q.lower * factor), whole(q.upper * factor)) {
            low = Some(low.map_or(l, |x| x.min(l)));
            high = Some(high.map_or(u, |x| x.max(u)));
        }
    }
    (low, high)
}

/// The year of a time value of year precision or finer (`+1995-00-00T00:00:00Z`,
/// `-2600-...`).
fn year_of(v: &Value) -> Option<i32> {
    if v.get("precision").and_then(Value::as_u64)? < 9 {
        return None;
    }
    let time = v.get("time")?.as_str()?;
    let sign = time.get(..1)?;
    let year: i32 = time.get(1..)?.split('-').next()?.parse().ok()?;
    match sign {
        "+" => Some(year),
        "-" => Some(-year),
        _ => None,
    }
}

/// P577, the earliest date.
fn year(entity: &Value) -> Option<i32> {
    best(entity, "P577").into_iter().filter_map(year_of).min()
}

/// P2339 when it is a number.
fn bgg_id(entity: &Value) -> Option<u64> {
    best(entity, "P2339")
        .into_iter()
        .find_map(Value::as_str)
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
}

/// P3962, up to five codes of 8 to 14 digits.
fn gtins(entity: &Value) -> Vec<String> {
    values(entity, "P3962")
        .into_iter()
        .filter_map(|(_, v)| v.as_str())
        .filter(|s| (8..=14).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit()))
        .take(MAX_GTINS)
        .map(String::from)
        .collect()
}

/// The item ids of the statements of `property` (all that are not deprecated).
fn linked(entity: &Value, property: &str) -> Vec<String> {
    values(entity, property)
        .into_iter()
        .filter_map(|(_, v)| v.get("id")?.as_str())
        .filter(|id| is_item_id(id))
        .map(String::from)
        .collect()
}

/// The ids of the mechanics (P4151) and genres (P136) of an item, each once.
fn linked_ids(entity: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in linked(entity, "P4151")
        .into_iter()
        .chain(linked(entity, "P136"))
    {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out.truncate(LABEL_BATCH);
    out
}

/// Labels of the linked items of `property`, in statement order, each once; an item
/// without a label is left out, never shown as its id.
fn linked_labels(entity: &Value, property: &str, labels: &Map<String, Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in linked(entity, property) {
        if let Some(l) = labels.get(&id).and_then(Value::as_str)
            && !out.iter().any(|o| o == l)
        {
            out.push(l.to_string());
        }
    }
    out
}

fn attributes(entity: &Value, labels: Option<&Map<String, Value>>) -> BoardGameAttributes {
    let (playtime_min_minutes, playtime_max_minutes) = playtime(entity);
    let names = |property: &str| {
        labels
            .map(|l| linked_labels(entity, property, l))
            .unwrap_or_default()
    };
    BoardGameAttributes {
        players_min: amount_in(entity, "P1872", &["1"]),
        players_max: amount_in(entity, "P1873", &["1"]),
        playtime_min_minutes,
        playtime_max_minutes,
        min_age: amount_in(entity, "P2899", &[YEARS_OLD, YEAR]),
        mechanics: names("P4151"),
        categories: names("P136"),
        bgg_id: bgg_id(entity),
        gtins: gtins(entity),
        ..BoardGameAttributes::default()
    }
}

/// One entity as a snapshot; `None` for a missing entity and, unless `id_as_title`, for one
/// without a label. `labels` holds the labels of linked items (details only).
fn snapshot(
    entity: &Value,
    language: &str,
    labels: Option<&Map<String, Value>>,
    id_as_title: bool,
) -> Option<Snapshot> {
    let id = entity.get("id")?.as_str().filter(|i| is_item_id(i))?;
    if entity.get("missing").is_some() {
        return None;
    }
    let title = match label(entity, language) {
        Some(l) => l.to_string(),
        None if id_as_title => id.to_string(),
        None => return None,
    };
    let english = text_in(entity.get("labels"), &["en"]).filter(|en| *en != title);
    let description = text_in(entity.get("descriptions"), &[language, "en"]);
    let attributes = attributes(entity, labels);
    // the QID is the only matching key: the BGG id and the GTINs are reference identifiers in
    // the attributes, because Wikidata has items that share them (DEC-195, WD-3, LB-36)
    let external_ids = vec![ExternalId::new(ID, id)];
    // a tie-break between rows only (DEC-611); far below 2^52 sitelinks
    #[allow(clippy::cast_precision_loss)]
    let sitelinks = entity
        .get("sitelinks")
        .and_then(Value::as_object)
        .map_or(0, Map::len) as f64;
    Some(Snapshot {
        connector: ID.into(),
        kind: KIND.into(),
        external_ids,
        title,
        original_title: english.map(String::from),
        year: year(entity),
        description: description.map(String::from),
        attributes: serde_json::to_value(attributes).unwrap_or_default(),
        // P18 points to Wikimedia Commons: no images in v1 (DEC-642)
        images: Vec::new(),
        complete: labels.is_some(),
        popularity: Some(sitelinks),
    })
}

fn check_kind(kind: &str) -> Result<(), ConnectorError> {
    if kind == KIND {
        Ok(())
    } else {
        Err(ConnectorError::Unsupported(format!("kind `{kind}`")))
    }
}

/// The item ids of a search answer, in its order.
fn hit_ids(page: &Value) -> Vec<String> {
    page.pointer("/query/search")
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter(|h| h.get("ns").and_then(Value::as_u64) == Some(0))
                .filter_map(|h| h.get("title")?.as_str())
                .filter(|t| is_item_id(t))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl Search for Wikidata {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        check_kind(&req.kind)?;
        if plain_words(&req.query).is_empty() {
            return Ok(Vec::new());
        }
        let language = language_of(req.language.as_deref());
        let url = self.search_url(&req.query, req.limit, ctx.lane)?;
        let page = get_api(ctx, "search", &url).await?;
        let ids = hit_ids(&page);
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let entities = self
            .entities(ctx, "search_entities", &ids, true, &language)
            .await?;
        // in the order of the search
        Ok(ids
            .iter()
            .filter_map(|id| snapshot(entities.get(id)?, &language, None, false))
            .collect())
    }
}

#[async_trait]
impl Details for Wikidata {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        check_kind(&req.kind)?;
        if !is_item_id(&req.id) {
            return Err(ConnectorError::NotFound);
        }
        let language = language_of(req.language.as_deref());
        let ids = [req.id.clone()];
        let entities = self.entities(ctx, "details", &ids, true, &language).await?;
        let entity = entities.get(&req.id).ok_or(ConnectorError::NotFound)?;
        if entity.get("missing").is_some() {
            return Err(ConnectorError::NotFound);
        }
        let labels = self.labels(ctx, &linked_ids(entity), &language).await?;
        snapshot(entity, &language, Some(&labels), true).ok_or(ConnectorError::NotFound)
    }
}

impl Connector for Wikidata {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }

    fn as_details(&self) -> Option<&dyn Details> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_item_id, language_of, plain_words};

    #[test]
    fn a_language_tag_gives_its_primary_subtag() {
        assert_eq!(language_of(Some("de-DE")), "de");
        assert_eq!(language_of(Some("pt_BR")), "pt");
        assert_eq!(language_of(Some("EN")), "en");
        assert_eq!(language_of(Some("x")), "en");
        assert_eq!(language_of(Some("de|fr")), "en");
        assert_eq!(language_of(None), "en");
    }

    #[test]
    fn only_item_ids_are_asked() {
        assert!(is_item_id("Q17271"));
        assert!(!is_item_id("Q017271"));
        assert!(!is_item_id("Q1234567890123"));
    }

    #[test]
    fn plain_words_collapse_whitespace_and_drop_bare_operators() {
        assert_eq!(
            plain_words("  Die Siedler   von Catan "),
            "Die Siedler von Catan"
        );
        assert_eq!(plain_words("-- !! :"), "");
    }
}
