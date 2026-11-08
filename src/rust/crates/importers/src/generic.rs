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
pub const CSV_COLUMNS: [&str; 10] = [
    "source_id",
    "kind",
    "title",
    "year",
    "status",
    "rating",
    "notes",
    "external_ids",
    "finished_dates",
    // added later; files without it still read (the column is optional)
    "lists",
];

/// Rows of a CSV file: `external_ids` as `ns:value;ns:value`, `finished_dates` and `lists`
/// as `a;b`.
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
                .map(|v| crate::cell::unguard(v.trim()))
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
                        event_id: None,
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
            lists: get("lists")
                .map(|v| {
                    v.split(';')
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
        })
    }))
}

/// Rows as CSV with the columns of [`CSV_COLUMNS`] (the CSV export). CSV keeps the dates of
/// finished views only (`finished_dates`); episodes and positions need JSON Lines. A `;`
/// inside a list name becomes `,` (the separator of the column).
#[must_use]
pub fn csv_text(rows: &[ImportRow]) -> String {
    let mut w = ::csv::Writer::from_writer(Vec::new());
    let _ = w.write_record(CSV_COLUMNS);
    for r in rows {
        let finished: Vec<&str> = r
            .history
            .iter()
            .filter(|h| h.progress.as_ref().is_none_or(|p| p["type"] == "finished"))
            .map(|h| h.date.as_str())
            .collect();
        let ids: Vec<String> = r
            .external_ids
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect();
        let lists: Vec<String> = r.lists.iter().map(|l| l.replace(';', ",")).collect();
        // free-text cells are spreadsheet-safe (UX3-5); numbers and dates are our own
        let text = |v: &str| crate::cell::guard(v).into_owned();
        let _ = w.write_record([
            text(&r.source_id),
            text(&r.kind),
            text(&r.title),
            r.year.map(|y| y.to_string()).unwrap_or_default(),
            text(r.status.as_deref().unwrap_or_default()),
            r.rating.map(|v| v.to_string()).unwrap_or_default(),
            text(r.notes.as_deref().unwrap_or_default()),
            text(&ids.join(";")),
            finished.join(";"),
            text(&lists.join(";")),
        ]);
    }
    w.into_inner()
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
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

    #[test]
    fn csv_export_reads_back() {
        let row = ImportRow {
            source_id: "s1".into(),
            kind: "movie".into(),
            title: "Heat, the film".into(),
            year: Some(1995),
            external_ids: [("tmdb".to_string(), "949".to_string())].into(),
            status: Some("completed".into()),
            rating: Some(90),
            notes: Some("line one\nline \"two\"".into()),
            history: vec![
                HistoryEntry {
                    date: "2024-05-01".into(),
                    progress: None,
                    event_id: None,
                },
                HistoryEntry {
                    date: "2024-06-01".into(),
                    progress: Some(
                        serde_json::json!({"type": "episode", "season": 1, "episode": 2}),
                    ),
                    event_id: None,
                },
            ],
            lists: vec!["Picks".into(), "A;B".into()],
        };
        let text = csv_text(std::slice::from_ref(&row));
        assert!(text.starts_with(
            "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates,lists\n"
        ));
        let back = rows(csv(std::io::Cursor::new(text)));
        let back = back[0].as_ref().unwrap();
        assert_eq!(back.title, row.title);
        assert_eq!(back.notes, row.notes);
        assert_eq!(back.external_ids, row.external_ids);
        assert_eq!(back.history.len(), 1, "episodes need JSON Lines");
        assert_eq!(back.lists, vec!["Picks".to_string(), "A,B".into()]);
    }

    /// UX3-5: a free-text cell a spreadsheet would run as a formula gets a leading `'`, and
    /// our own reader takes it off again, so an export still reads back unchanged.
    #[test]
    fn formula_cells_are_neutralised_and_read_back() {
        let row = ImportRow {
            source_id: "@s1".into(),
            kind: "movie".into(),
            title: "=HYPERLINK(\"http://x.example\",\"Heat\")".into(),
            year: None,
            external_ids: std::collections::BTreeMap::new(),
            status: Some("+planned".into()),
            rating: None,
            notes: Some("-1 star".into()),
            history: Vec::new(),
            lists: vec!["@home".into(), "'=quoted".into()],
        };
        let text = csv_text(std::slice::from_ref(&row));
        let line = text.lines().nth(1).unwrap();
        for cell in [
            "'@s1",
            "'=HYPERLINK",
            "'+planned",
            "'-1 star",
            "'@home;'=quoted",
        ] {
            assert!(line.contains(cell), "{cell}: {line}");
        }
        let back = rows(csv(std::io::Cursor::new(text)));
        let back = back[0].as_ref().unwrap();
        assert_eq!(back.title, row.title);
        assert_eq!(back.notes, row.notes);
        assert_eq!(back.source_id, row.source_id);
        assert_eq!(back.lists, row.lists);
    }
}
