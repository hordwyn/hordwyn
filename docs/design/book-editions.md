# Design: book editions (1.1.0, package A, issue #20)

Status: **draft for the owner's review** (2026-11-20). Nothing is built. Derived from
`docs/research/book-editions.md` (cited as R-n for its findings in section 6, [n] for its
references). Each question has one recommended option (marked **Recommended**) and the
alternatives. The owner's rulings become DEC lines; then the package is cut into test-first
steps.

## 0. Where we start (code as of v1.0.1)

- A book is one catalogue item per **work** (`openlibrary:OL...W`), shared by every member.
  Its `attributes.page_count` is Open Library's `number_of_pages_median` (provenance
  `openlibrary`, shown as an estimate) or a value a person set with "Change" (provenance
  `user`, shared, kept by refresh). An ISBN search resolves the edition only to reach its work;
  the edition id, pages, format and publisher are thrown away.
- Progress of a book is `Paged { page }` events (user + item, no entry id); percent is derived
  from the item's page total; stats count forward page deltas per item; completions are
  status changes plus `completed_at` on the entry.
- `audiobook` is its own kind (`Timed`, minutes), with no source.
- Export: JSONL `library-export` version 1, `deny_unknown_fields` on rows.

## 1. What an edition is in Hordwyn

**Q1. A record of its own, or attributes on the library entry?**

- **Recommended: a shared edition record linked to the work item** (table `book_editions`),
  and an optional `edition_id` on the library entry. An edition is the LRM manifestation
  (R-1): it has its own identity (ISBN, Open Library `OL...M`, R-2, R-4), is the same for every
  reader of that copy, and carries its own pages, cover, title and language (R-5, R-13). Two
  members reading the same paperback share one record, like they share the work.
- Alternative A: copy the edition's fields onto the entry (pages, ISBN, format, publisher).
  No new table, but no shared correction, duplicates per member, and the cover and the edition
  picker have nothing to point at.
- Alternative B: an edition as a catalogue item of its own kind, linked to the work. Reuses
  covers, provenance and refresh, but every library, search and stats query that knows "one
  item = one work" would need to fold editions back into works.

**Q2. Which fields?** (from Open Library's edition record, R-5)

| Field | Source | Notes |
|---|---|---|
| `id` | ours | uuid |
| `item_id` | ours | the work item; `ON DELETE CASCADE` |
| `external_ids` | `openlibrary:OL...M`, `isbn:<13>` (both ISBN forms resolve to one record, R-4) | unique per edition; none required (R-3) |
| `title`, `subtitle` | edition `title`, `subtitle` | may differ from the work (translation, R-13) |
| `edition_name` | `edition_name` | free text ("Collector's Edition"), 10 of 38 for the OT-26 work |
| `format` | `physical_format`, mapped | enum `hardcover`, `paperback`, `ebook`, `audio`, `other`, `unknown`; the raw text kept |
| `publishers` | `publishers` | list |
| `published` | `publish_date` | free text kept as is, plus a parsed `year` when plausible (DEC-859 rule) |
| `pages` | `pagination` parsed, else `number_of_pages` | numbered pages (R-7); `pages_from` = `pagination`, `number_of_pages` or `user` |
| `language` | `languages[0]` | ISO 639 code |
| `cover` | `covers[0]` | same cover rules as items (DEC-736) |
| `provenance` | ours | per field, `user` wins over refresh, like items |

**Q3. Is an audiobook an edition of the book?**

- **Recommended: no, in 1.1.0. Audio stays the `audiobook` kind** (minutes). Open Library's
  audio editions are left out of the book's edition list: their `number_of_pages` is a disc
  count (R-8) and they have no duration, so they would break page totals. The picker says
  "Audiobook? Add it as an audiobook" and links the audiobook search.
- Alternative: an audio edition of the book with a duration, progress in time on the same
  entry (Hardcover, StoryGraph do this, R-8, R-11). Changes the progress model of `book`
  (pages or seconds per edition) and stats; a 2.0 candidate.

## 2. Where the data comes from

**Q4. Sources and order.**

- **Recommended: Open Library first, by hand second; no Wikidata for editions.**
  - ISBN: `/isbn/{isbn}.json` already answers the edition; keep it (no extra request).
  - The edition list of a work: `/works/{id}/editions.json?limit=200` (one request, paged with
    `links.next` beyond that, R-6), fetched when a person opens the picker, cached on the work
    (editions stored as records only when someone picks one, so the table holds what is used).
  - By hand: format, publisher, year, pages, ISBN (checked with the existing `isbn13()`).
  - Wikidata: too sparse (13 percent of works have any edition item, R-9); keep it for the
    work's year only (OT-36).
- Alternative: store every edition of a work on the first fetch (482 rows for The Hobbit):
  more data than anyone reads, more refresh work.
- BookBrainz: not now (alpha, small, R-10). Hardcover, Goodreads, StoryGraph, Libib: not
  usable as sources (terms, R-15).

**Q5. Which page number counts as the edition's total?**

- **Recommended: the numbered pages** (the leading number of `pagination`, "433 p."), else
  `number_of_pages`; a person can correct it (provenance `user`). Matches what a reader sees on
  the last page and Hardcover's rule (R-7). The parser takes "N p", "N pages", "N, [1] p",
  skips roman preliminaries ("xii, 433 p." gives 433) and refuses audio extents.
- Alternative: `number_of_pages` always (simpler; 448 for the OT-26 copy instead of 433).

## 3. What the edition changes

**Q6. The page total of an entry.**

- **Recommended:** an entry with an edition uses the edition's pages; without one, the work's
  page total as today (the estimate or the shared "Change" value). The "Change" control on a
  book with an edition corrects the edition's pages (shared, `user`), not the work's.
- Alternative: keep one page total per entry (personal, not shared). Fits "my copy", but
  members of one household reading the same copy correct it twice.

**Q7. Changing the edition mid-read (progress, stats, goals).**

Progress is stored as page numbers; page 200 of 400 is not page 200 of 600 (R-12).

- **Recommended: each paged event records the edition it was read in; a change carries the
  position over by percent and that step counts no pages.**
  - `Progress::Paged` gets an optional `edition` (the edition id; jsonb, so no column change;
    absent on old events = "the work", as today).
  - On a change while reading, the dialog says "You are on page 200 of 400 (50 %). Continue on
    page 300 of 600 in the new edition?" with the page editable; Save logs a `Paged` event in
    the new edition.
  - Stats count forward deltas only between events of the same edition, so the carried-over
    step adds nothing and pages read stay true per copy.
  - Completions, goals of books, streaks: unchanged (they count statuses, not pages).
- Alternative A: rescale the old events (rewrites history; undo and export get confusing).
- Alternative B: no carry-over; the person logs the new page themselves (StoryGraph users do
  this, R-12); the first event in the new edition would count as a jump unless stats know the
  edition anyway, so B needs the same `edition` field and saves only the dialog.

**Q8. One edition per entry, or one per read?**

- **Recommended: one per entry, changeable**; every event keeps the edition it was logged in
  (Q7), so a past read is still known to have been the paperback. A per-read edition comes with
  #11/#14 (counts of passes) if the owner wants it there.
- Alternative: an edition per read (Hardcover `user_book_reads`, R-11): needs a "read" record
  Hordwyn does not have yet.

**Q9. Year.**

- **Recommended:** the item keeps the work's first publication (DEC-859, stats and filters
  use it); the edition's year shows beside it on the item page ("first published 2015; this
  edition 2019"). OT-36 (#16) lets a person correct the work's year.
- Alternative: show the edition's year as the item's year for that person.

**Q10. Title, language and cover shown to a person who picked an edition.**

- **Recommended:** the work title stays the title everywhere (library, history, search); the
  item page's edition line shows the edition's title when it differs ("Edition: Der Hobbit,
  Taschenbuch, dtv 2012, 383 pages"). The cover of the edition replaces the work's cover for
  that person (library card, item page), when the edition has one.
- Alternative: the edition's title as the main title for that person (answers part of
  OT-34, but the same book then has two names in shared views such as "Watched with" rows and
  lists).

## 4. Search and adding

**Q11. Search flows.**

- ISBN typed in the palette (any form, with or without hyphens): the result row is that
  edition: "Title - Hardcover, Bloomsbury, 2019, 433 pages" (R-2). Adding it adds the work
  item and the entry with that edition set.
- Title search: work rows as today (R-6: search.json gives one edition per work). After
  adding, the entry has no edition; the item page and the log sheet show "Edition: not set -
  Choose" (one click opens the picker).
- **Recommended:** no forced pick while adding (the edition is optional, OT-26).
- Alternative: ask "Which edition?" right after adding a book from a title search (one more
  step for every book, most people skip it).

**Q12. The picker.**

- A dialog with the work's editions (Q4) filtered to readable formats (Q3), newest first,
  filter by format and language, a text box that takes an ISBN or a publisher; each row:
  cover, title if different, format, publisher, year, pages; "Enter by hand" and "No specific
  edition" at the end.

## 5. Import, export, API

**Q13.**

- **Recommended:** export format stays version 1 with an optional `edition` object on a row
  (`openlibrary`, `isbn13`, `format`, `publisher`, `year`, `pages`, `title`) written only when
  set (`skip_serializing_if`); the import reads it and resolves or creates the edition. Paged
  history entries carry the optional `edition` key (Q7). REST: `edition` on the entry DTO
  (additive) and `PUT /library/{item}/edition`.
- Note: a 1.0.x build rejects a 1.1.0 export that has editions (`deny_unknown_fields`);
  moving data back to an older version is not promised. Say so in the release notes.
- Alternative: format version 2 (honest about the change, but every reader of version 1
  breaks, and 1.1.0 is a minor).

## 6. Migration (draft only; 0002, additive)

```sql
-- 0002_book_editions.sql (DRAFT, not in migrations/ until the owner agreed the design)
CREATE TABLE book_editions (
  id uuid PRIMARY KEY,
  item_id uuid NOT NULL REFERENCES catalog_items(id) ON DELETE CASCADE,
  title text, subtitle text, edition_name text,
  format text NOT NULL DEFAULT 'unknown'
    CHECK (format IN ('hardcover','paperback','ebook','audio','other','unknown')),
  format_text text,
  publishers text[] NOT NULL DEFAULT '{}',
  published text, year int,
  pages int CHECK (pages > 0), pages_from text,
  language text,
  cover_asset text, cover_url text,
  provenance jsonb NOT NULL DEFAULT '{}',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE book_edition_ids (
  edition_id uuid NOT NULL REFERENCES book_editions(id) ON DELETE CASCADE,
  source text NOT NULL, value text NOT NULL,
  PRIMARY KEY (source, value)
);
CREATE INDEX book_editions_item ON book_editions(item_id);
ALTER TABLE library_entries
  ADD COLUMN edition_id uuid REFERENCES book_editions(id) ON DELETE SET NULL;
```

**Q14. Existing entries.**

- **Recommended:** the migration sets nothing (every entry keeps working with the work's page
  total, as today). A `cli editions match [--apply]` (dry run by default, like `match-books`)
  sets the edition of entries whose item was added through an ISBN (`isbn:` external id)
  and lists what it would do; the owner runs it after the upgrade. Page totals a person set
  (`user`) stay on the work and are shown in the dry run so nothing is lost.
- Alternative: let the migration guess editions (SQL cannot reach Open Library; would need a
  job anyway).
- Upgrade test: the seed gets a v1.0.1 book entry with progress and a `user` page total;
  `upgrade-test` from v1.0.1 must keep both and show the entry with no edition.

## 7. Worked example

The owner's book started on 2026-11-20 is still to be named (question 1 of the review). Until
then the OT-26 copy (ISBN 978-1-5476-0417-3) stands in: the ISBN search finds the edition
record, `pagination` gives 433 numbered pages (the work's median says 448, R-7); the entry
started without an edition carries the hand-set total (`user`) and switches to the edition
with "Continue on page N" (Q7).

## 8. Steps once agreed (test-first, one commit each)

1. Domain and migration 0002 (`UPDATE_SCHEMA=1`), store, the pagination parser.
2. Open Library: edition from `/isbn/`, the edition list, the format map.
3. App: set/clear the edition, page total from the edition, `Paged.edition`, stats per edition.
4. Web: edition line, picker, change dialog with carry-over; ISBN result rows.
5. Import/export, REST, `cli editions match`, upgrade seed and `upgrade-test` from v1.0.1.
6. Docs (user guide, release notes), CHANGELOG.
