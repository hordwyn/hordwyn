//! Meilisearch adapter (meilisearch-sdk 0.33.0).
//!
//! The master key is used once, at start-up, to configure indexes and to create (or reuse) a
//! scoped API key limited to this deployment's indexes and to search + document actions.
//! All later requests use the scoped key; no key ever leaves the server (D-13).

use std::time::Duration;

use app::ports::{PortError, SearchDoc, SearchHit, SearchIndex, SearchQuery};
use async_trait::async_trait;
use meilisearch_sdk::client::Client;
use meilisearch_sdk::key::{Action, KeyBuilder};
use meilisearch_sdk::task_info::TaskInfo;
use serde_json::{Map, Value};

use crate::{filterable, is_private, searchable};

fn me(e: impl std::fmt::Display) -> PortError {
    PortError(format!("meilisearch: {e}"))
}

/// Quote a filter value for Meilisearch's filter syntax.
#[must_use]
pub fn quote(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

pub struct MeiliIndex {
    client: Client,
    prefix: String,
    timeout: Duration,
}

impl MeiliIndex {
    /// Configure indexes with the master key and switch to a scoped key.
    ///
    /// # Errors
    /// Unreachable server, wrong master key, failed settings tasks.
    pub async fn connect(url: &str, master_key: &str, prefix: &str) -> Result<Self, PortError> {
        let admin = Client::new(url, Some(master_key)).map_err(me)?;
        let timeout = Duration::from_secs(30);
        for name in crate::INDEXES {
            let uid = format!("{prefix}{name}");
            let created = admin.create_index(&uid, Some("id")).await.map_err(me)?;
            // "index already exists" is fine; any other failure is reported by the settings task
            let _ = created
                .wait_for_completion(&admin, None, Some(timeout))
                .await;
            let index = admin.index(&uid);
            let mut filters: Vec<&str> = filterable(name).to_vec();
            filters.push("owner");
            wait(
                &admin,
                index
                    .set_filterable_attributes(&filters)
                    .await
                    .map_err(me)?,
                timeout,
            )
            .await?;
            wait(
                &admin,
                index
                    .set_searchable_attributes(searchable(name))
                    .await
                    .map_err(me)?,
                timeout,
            )
            .await?;
            if name == crate::CATALOG {
                popularity_rule(&admin, &index, timeout).await?;
            }
        }
        let key = scoped_key(&admin, prefix).await?;
        Ok(Self {
            client: Client::new(url, Some(key)).map_err(me)?,
            prefix: prefix.to_string(),
            timeout,
        })
    }

    fn uid(&self, index: &str) -> String {
        format!("{}{index}", self.prefix)
    }

    /// Number of documents and on-disk size, for the storage report.
    ///
    /// # Errors
    /// Server errors.
    pub async fn stats(&self) -> Result<(u64, u64), PortError> {
        let mut docs = 0;
        for name in crate::INDEXES {
            let s = self
                .client
                .index(self.uid(name))
                .get_stats()
                .await
                .map_err(me)?;
            docs += u64::try_from(s.number_of_documents).unwrap_or(0);
        }
        let all = self.client.get_stats().await.map_err(me)?;
        Ok((docs, u64::try_from(all.database_size).unwrap_or(0)))
    }

    /// Remove every document of an index (full reindex).
    ///
    /// # Errors
    /// Server errors.
    pub async fn clear(&self, index: &str) -> Result<(), PortError> {
        let t = self
            .client
            .index(self.uid(index))
            .delete_all_documents()
            .await
            .map_err(me)?;
        wait(&self.client, t, self.timeout).await
    }
}

/// The source's popularity as the last ranking rule of the catalog (DEC-611): it only orders
/// documents the built-in rules leave equal (same words, typos, proximity, attribute, word
/// position and exactness), for example two titles that both start with the query. The
/// server's own list is read and extended, so its built-in rules stay as its version has
/// them. Every catalog document carries the field as a number (0 when the source gives
/// none): Meilisearch leaves the order undefined for documents without it.
const POPULARITY_RULE: &str = "popularity:desc";

async fn popularity_rule(
    admin: &Client,
    index: &meilisearch_sdk::indexes::Index,
    timeout: Duration,
) -> Result<(), PortError> {
    let mut rules = index.get_ranking_rules().await.map_err(me)?;
    if rules.iter().any(|r| r == POPULARITY_RULE) {
        return Ok(());
    }
    rules.push(POPULARITY_RULE.to_string());
    wait(
        admin,
        index.set_ranking_rules(&rules).await.map_err(me)?,
        timeout,
    )
    .await
}

async fn wait(client: &Client, t: TaskInfo, timeout: Duration) -> Result<(), PortError> {
    let task = t
        .wait_for_completion(client, Some(Duration::from_millis(20)), Some(timeout))
        .await
        .map_err(me)?;
    if task.is_failure() {
        return Err(PortError(format!("meilisearch task failed: {task:?}")));
    }
    Ok(())
}

/// Stable uid per prefix, so restarts reuse the key instead of creating new ones.
fn key_uid(prefix: &str) -> String {
    let h = blake3::hash(format!("search-key:{prefix}").as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&h.as_bytes()[..16]);
    uuid::Builder::from_random_bytes(b).into_uuid().to_string()
}

async fn scoped_key(admin: &Client, prefix: &str) -> Result<String, PortError> {
    let uid = key_uid(prefix);
    if let Ok(k) = admin.get_key(&uid).await {
        return Ok(k.key);
    }
    let mut b = KeyBuilder::new();
    b.with_uid(&uid)
        .with_name(format!("server {prefix}*"))
        .with_actions([
            Action::Search,
            Action::DocumentsAdd,
            Action::DocumentsGet,
            Action::DocumentsDelete,
            Action::TasksGet,
            Action::IndexesGet,
            Action::StatsGet,
        ])
        .with_index(format!("{prefix}*"));
    Ok(admin.create_key(b).await.map_err(me)?.key)
}

fn to_doc(d: SearchDoc) -> Value {
    let mut m = match d.fields {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    m.insert("id".into(), Value::String(d.id));
    m.insert(
        "owner".into(),
        d.owner
            .map_or(Value::Null, |o| Value::String(o.0.to_string())),
    );
    Value::Object(m)
}

/// The server-side filter: the viewer's documents only on private indexes, plus the caller's
/// (allowlisted, quoted) filters.
#[must_use]
pub fn filter(q: &SearchQuery) -> String {
    let mut parts = Vec::new();
    if is_private(&q.index) {
        parts.push(format!("owner = {}", quote(&q.viewer.0.to_string())));
    }
    for (k, v) in &q.filters {
        parts.push(format!("{k} = {}", quote(v)));
    }
    parts.join(" AND ")
}

#[async_trait]
impl SearchIndex for MeiliIndex {
    async fn upsert(&self, index: &str, docs: Vec<SearchDoc>) -> Result<(), PortError> {
        if docs.is_empty() {
            return Ok(());
        }
        let docs: Vec<Value> = docs.into_iter().map(to_doc).collect();
        let t = self
            .client
            .index(self.uid(index))
            .add_or_replace(&docs, Some("id"))
            .await
            .map_err(me)?;
        wait(&self.client, t, self.timeout).await
    }

    async fn delete(&self, index: &str, ids: Vec<String>) -> Result<(), PortError> {
        if ids.is_empty() {
            return Ok(());
        }
        let t = self
            .client
            .index(self.uid(index))
            .delete_documents(&ids)
            .await
            .map_err(me)?;
        wait(&self.client, t, self.timeout).await
    }

    async fn search(&self, q: SearchQuery) -> Result<Vec<SearchHit>, PortError> {
        crate::check(&q)?;
        let filter = filter(&q);
        let index = self.client.index(self.uid(&q.index));
        let mut s = index.search();
        s.with_query(&q.text).with_limit(q.limit.clamp(1, 100));
        if !filter.is_empty() {
            s.with_filter(&filter);
        }
        let res = s.execute::<Value>().await.map_err(me)?;
        Ok(res
            .hits
            .into_iter()
            .map(|h| {
                let mut fields = h.result;
                let id = fields
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if let Value::Object(m) = &mut fields {
                    m.remove("owner");
                }
                SearchHit { id, fields }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::UserId;

    #[test]
    fn filters_quote_values_and_always_scope_private_indexes() {
        let viewer = UserId(uuid::Uuid::nil());
        let q = SearchQuery {
            index: "library".into(),
            text: String::new(),
            viewer,
            filters: vec![("status".into(), "x\" OR owner = \"y".into())],
            limit: 5,
        };
        assert_eq!(
            filter(&q),
            "owner = \"00000000-0000-0000-0000-000000000000\" AND status = \"x\\\" OR owner = \\\"y\""
        );
        let c = SearchQuery {
            index: "catalog".into(),
            filters: vec![],
            ..q
        };
        assert_eq!(filter(&c), "", "the shared catalog has no owner scope");
    }

    #[test]
    fn key_uid_is_stable_and_a_uuid() {
        assert_eq!(key_uid("a_"), key_uid("a_"));
        assert_ne!(key_uid("a_"), key_uid("b_"));
        assert!(uuid::Uuid::parse_str(&key_uid("a_")).is_ok());
    }
}
