//! Our own format (Q10), also produced by the export: JSON Lines (a header line, then one
//! [`ImportRow`] per line) or CSV with the same fields. Both are parsed as streams, row by row.

use std::io::{BufRead, Read};

use crate::row::{GENERIC_FORMAT, GENERIC_VERSION, Header, HistoryEntry, ImportRow, RowError};

pub type Parsed = Result<ImportRow, RowError>;

/// Rows of a JSON Lines file. The first non-empty line must be the header.
pub fn jsonl<R: BufRead + 'static>(input: R) -> Box<dyn Iterator<Item = Parsed>> {
    let mut seen_header = false;
    Box::new(input.lines().enumerate().filter_map(move |(i, line)| {
        let n = i as u64 + 1;
        let line = match line {
            Ok(l) => l,
            Err(e) => return Some(Err(err(n, None, format!("read error: {e}")))),
        };
        if line.trim().is_empty() {
            return None;
        }
        if !seen_header {
            seen_header = true;
            return match serde_json::from_str::<Header>(&line) {
                Ok(h) if h.format == GENERIC_FORMAT && h.version == GENERIC_VERSION => None,
                Ok(h) => Some(Err(err(
                    n,
                    None,
                    format!("unsupported format {} version {}", h.format, h.version),
                ))),
                Err(e) => Some(Err(err(n, None, format!("missing header: {e}")))),
            };
        }
        Some(serde_json::from_str::<ImportRow>(&line).map_err(|e| {
            let id = serde_json::from_str::<serde_json::Value>(&line)
                .ok()
                .and_then(|v| v["source_id"].as_str().map(String::from));
            err(n, id, e.to_string())
        }))
    }))
}

fn err(line: u64, source_id: Option<String>, message: String) -> RowError {
    RowError {
        line,
        source_id,
        message,
    }
}

/// CSV columns of the generic format (header row required).
pub const CSV_COLUMNS: [&str; 9] = [
    "source_id",
    "kind",
    "title",
    "year",
    "status",
    "rating",
    "notes",
    "external_ids",
    "finished_dates",
];

/// Rows of a CSV file: `external_ids` as `ns:value;ns:value`, `finished_dates` as
/// `date;date`.
pub fn csv<R: Read + 'static>(input: R) -> Box<dyn Iterator<Item = Parsed>> {
    let reader = ::csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input);
    Box::new(reader.into_records().enumerate().map(|(i, rec)| {
        // header is line 1
        let n = i as u64 + 2;
        let rec = rec.map_err(|e| err(n, None, e.to_string()))?;
        let get = |c: &str| -> Option<String> {
            let idx = CSV_COLUMNS.iter().position(|x| *x == c)?;
            rec.get(idx)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(String::from)
        };
        let source_id =
            get("source_id").ok_or_else(|| err(n, None, "source_id is empty".into()))?;
        let parse_num = |c: &str| -> Result<Option<i64>, RowError> {
            get(c)
                .map(|v| {
                    v.parse::<i64>().map_err(|_| {
                        err(
                            n,
                            Some(source_id.clone()),
                            format!("{c} `{v}` is not a number"),
                        )
                    })
                })
                .transpose()
        };
        let year = parse_num("year")?.and_then(|y| i32::try_from(y).ok());
        let rating = parse_num("rating")?
            .map(|r| {
                u8::try_from(r).ok().filter(|r| *r <= 100).ok_or_else(|| {
                    err(
                        n,
                        Some(source_id.clone()),
                        format!("rating {r} is not 0..100"),
                    )
                })
            })
            .transpose()?;
        let external_ids = get("external_ids")
            .map(|v| {
                v.split(';')
                    .filter_map(|p| p.split_once(':'))
                    .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let history = get("finished_dates")
            .map(|v| {
                v.split(';')
                    .map(str::trim)
                    .filter(|d| !d.is_empty())
                    .map(|d| HistoryEntry {
                        date: d.to_string(),
                        progress: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(ImportRow {
            kind: get("kind")
                .ok_or_else(|| err(n, Some(source_id.clone()), "kind is empty".into()))?,
            title: get("title")
                .ok_or_else(|| err(n, Some(source_id.clone()), "title is empty".into()))?,
            source_id,
            year,
            external_ids,
            status: get("status"),
            rating,
            notes: get("notes"),
            history,
        })
    }))
}

/// The header line of an export.
#[must_use]
pub fn header_line() -> String {
    serde_json::to_string(&Header {
        format: GENERIC_FORMAT.into(),
        version: GENERIC_VERSION,
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(it: Box<dyn Iterator<Item = Parsed>>) -> Vec<Parsed> {
        it.collect()
    }

    #[test]
    fn parses_json_lines_and_reports_bad_rows() {
        let text = format!(
            "{}\n{}\n\n{}\n{}\n",
            header_line(),
            r#"{"source_id":"1","kind":"movie","title":"Heat","year":1995,"external_ids":{"tmdb":"949"},"status":"completed","rating":90,"history":[{"date":"2024-05-01"}]}"#,
            r#"{"source_id":"2","kind":"movie"}"#,
            r#"{"source_id":"3","kind":"book","title":"Dune","bogus":1}"#,
        );
        let out = rows(jsonl(std::io::Cursor::new(text)));
        assert_eq!(out.len(), 3);
        let first = out[0].as_ref().unwrap();
        assert_eq!(first.title, "Heat");
        assert_eq!(first.external_ids["tmdb"], "949");
        let e = out[1].as_ref().unwrap_err();
        assert_eq!(
            (e.line, e.source_id.as_deref()),
            (4, Some("2")),
            "blank line 3 counts"
        );
        assert!(out[2].as_ref().unwrap_err().message.contains("bogus"));
    }

    #[test]
    fn requires_the_header() {
        let out = rows(jsonl(std::io::Cursor::new(
            r#"{"source_id":"1","kind":"movie","title":"Heat"}"#.to_string(),
        )));
        assert!(
            out[0]
                .as_ref()
                .unwrap_err()
                .message
                .contains("missing header")
        );
    }

    #[test]
    fn parses_csv() {
        let text = "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates\n\
                    a1,movie,Heat,1995,completed,90,great,tmdb:949;imdb:tt0113277,2024-05-01;2025-01-02\n\
                    a2,movie,Ronin,abc,,,,,\n";
        let out = rows(csv(std::io::Cursor::new(text.to_string())));
        let r = out[0].as_ref().unwrap();
        assert_eq!(r.external_ids["imdb"], "tt0113277");
        assert_eq!(r.history.len(), 2);
        assert_eq!(r.rating, Some(90));
        let e = out[1].as_ref().unwrap_err();
        assert_eq!(e.line, 3);
        assert!(e.message.contains("year"));
    }
}
