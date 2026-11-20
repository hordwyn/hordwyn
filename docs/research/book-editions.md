# Research: book editions (1.1.0, package A)

Date read for every web source: 2026-11-20. Research only: the design is written separately
from the findings at the end. Questions come from docs/handoff/1.1.0-go.md, section
"1. Research". Owner cases: OT-26, OT-34, OT-36 (docs/handoff/owner-test-2026-11-15.md) and
the book editions note in docs/handoff/v2-notes.md.

Method. Public documentation and terms pages were read. Where a page could not be fetched,
this is said and the fallback is named. A few live API answers were read on 2026-11-20 with a
generic client name and no contact, at most one request per 1.5 s (Open Library: 9 requests,
Wikidata: about 12, BookBrainz: 5, Hardcover docs pages: 8). Inline references are [n]; the
list is at the end.

## 1. Licence and terms check

Project rule (KICKOFF, DEC-169): no GPL code; reading public API documentation and terms of a
GPL-family service is fine. Reading a standard or a manual is always fine. Using data depends
on the data licence. Copying code is never done from GPL projects (and no code is copied
from anything here).

| Source | Licence / terms as read | Read docs | Use data | Copy code |
|---|---|---|---|---|
| IFLA LRM (2017, rev. 2017-12) [1] | Document under CC BY 4.0 ("This work is licensed under the Creative Commons Attribution 4.0 International (CC BY 4.0) license") | yes | not data; cite the model with attribution | n/a |
| FRBR final report (1997, amended 2009) [2] | No licence statement found in the PDF text; IFLA publication | yes | n/a (we only cite it) | n/a |
| RDA / RDA Toolkit [3], RDA Registry [4] | Toolkit: published by the RDA Copyright Holders (ALA, CFLA, CILIP), subscription site, use under the "RDA Toolkit License Agreement" and a copyright and permissions policy. Registry (element names and definitions): "licensed under a Creative Commons Attribution 4.0 International License" | Registry yes; Toolkit text not read (paywalled) | element names and definitions from the Registry may be quoted with attribution | n/a |
| MARC 21 Bibliographic, field 250 [5] | Library of Congress public page; terms of reuse not checked | yes | n/a (format definition only) | n/a |
| ISBN Users' Manual, 7th ed. (2017) [6], ISBN site [7] | "(c) 2017 by International ISBN Agency"; "Permission is granted for the reprinting of any material in this brochure, subject to due acknowledgement of the source and to a copy, preferably digital, being sent to the International ISBN Agency." | yes | rules may be cited; quoting is allowed with acknowledgement | n/a |
| Open Library data and API [8][9][10][11] | Data: "The Internet Archive does not assert any new copyright or other proprietary rights over any of the material in the Open Library database. There may be existing rights issues on some contributions and in some jurisdictions." No licence text (our tag `unspecified-open`, docs/connectors/openlibrary.md). API: 1 request/s anonymous, 3/s "with User-Agent and email"; "Cache responses whenever possible"; do not "Make hundreds of single-book requests (use search.json for batch results)" nor use it "as a backend for high-traffic commercial services". Service software is AGPL-3.0 (DEC-106, DEC-169) | yes (API docs and terms only) | yes, display; already used in v1 | never (AGPL) |
| Wikidata data and API [15][16] | Structured data (main, Property, Lexeme, EntitySchema namespaces) "is released into the public domain under" CC0; other text CC BY-SA 4.0; site use under the Wikimedia Terms of Use. User-Agent policy: every request needs a descriptive User-Agent with contact ("client name and version, then contact information in parentheses"); generic agents "may be blocked without notice", a missing or generic one can get 403 | yes | yes (CC0), any use | n/a |
| BookBrainz [18][19][20][21] | Data: "The BookBrainz core data is licensed under the Creative Commons CC0 license" (author, edition group, edition, publisher, work records and relationships); "Other publically available data is licensed under the Creative Commons CC BY-SA license". Site marked "Alpha Software". Code (bookbrainz-site): GPL-2.0. No API terms page found | yes (help, licensing, API answers) | core data yes (CC0) | never (GPL-2.0) |
| Goodreads [22][23] | Public API: no new developer keys since 8 Dec 2020, "plans to retire the current version". Help and terms pages could not be fetched (the help site is a script-only page; "CSS Error" for the fetcher). Relied on search-result summaries of help and forum threads | yes (what could be read) | no API for us; no data reuse | n/a (closed source) |
| StoryGraph [24][25][26][27] | No public API (a roadmap request "An API" from 2021 is open, marked long-term; from search summary). Terms page answered 403 to the fetcher, not read. Help article and roadmap posts were read | yes | no | n/a (closed source) |
| Hardcover API [28]-[33] | GraphQL API in beta: "heavily in flux", "We may reset tokens without notice while in beta"; each request runs as the token's user; "By using the API, you agree not to use to train large language models that are made publicly available or used commercially"; catalogue: "we make no copyright or proprietary rights over this data"; a commercial product or public website "may not use data owned by users" except on behalf of that user; images are user-uploaded (keep a DMCA takedown policy when shown publicly). Limits: free plan 5,000 requests a day, burst 10, 60 per minute | yes | catalogue data possible per user token (each person or operator needs a token); not needed for this package | n/a (closed source) |
| Libib [34][35] | Terms forbid access through "bots, scripts, or AI agents" and to "scrape, or otherwise access the Service by any means not expressly provided or authorized by Libib". No public API found | yes | no | n/a (closed source) |
| Calibre [36][37] | Code GPL-3.0. Manual pages "(c) Copyright Kovid Goyal", no licence stated for the manual | yes (manual only) | n/a | never (GPL-3.0) |

Result: every model and rule below comes from documents we may read and cite. Data sources
usable for editions are Open Library (in use), Wikidata (CC0) and BookBrainz core data (CC0).
Goodreads, StoryGraph, Libib and Calibre are read only for how they present editions.

## 2. The models: work, expression, manifestation, item

### 2.1 IFLA LRM and FRBR

IFLA LRM (2017) [1] merges FRBR, FRAD and FRSAD. It keeps the four FRBR entities, all disjoint:

- **Work (LRM-E2)**: "The intellectual or artistic content of a distinct creation". Abstract;
  it groups expressions "considered functional equivalents or near equivalents". "When the
  majority of users, for most general purposes, would regard the expression instances as
  being intellectually equivalent, then these expressions are considered to be expressions
  of the same work." Paraphrases, rewritings, adaptations for children, dramatizations,
  abstracts, digests and summaries are new works.
- **Expression (LRM-E3)**: "A distinct combination of signs conveying intellectual or
  artistic content": the actual words of a text. Typeface and page layout are excluded.
  "Any change in form (e.g., from written notation to spoken word) results in a new
  expression." A translation is a new expression. "If a text is revised or modified, the
  resulting expression is considered to be a new expression of the work." "Minor changes,
  such as corrections of spelling and punctuation" stay in the same expression.
  Illustrations, notes and glosses that are not integral to the work "are considered to be
  separate expressions of their own separate work(s)" (an aggregate).
- **Manifestation (LRM-E4)**: "A set of all carriers that are assumed to share the same
  characteristics as to intellectual or artistic content and aspects of physical form",
  defined by "the overall content and the production plan". A change in physical form
  (typeface, size of font, page layout, medium, container) makes a new manifestation. "A
  change in publisher, repackaging, etc." "may be considered a new manifestation". A change
  of content makes a new expression embodied in a new manifestation. "Certain minor
  variations or differences in packaging may not be considered bibliographically
  significant." Print on demand and downloads of one online file are the same manifestation.
- **Item (LRM-E5)**: "An object or objects carrying signs": one physical copy (or one file).
  Damage, a lost page or rebinding do not make a new manifestation. Example: "Numbered copy
  4 (of a limited edition of 50)".

FRBR [2] uses the same four entities (manifestation: "the physical embodiment of an
expression of a work"; "All copies produced that form part of the same set are considered
to be copies of the same manifestation"). FRBR notes that revised texts "will often be
indirectly identified as different expressions because the variation is apparent from ...
an edition statement".

**Where "edition" sits.** Neither model has an entity called edition. In LRM the edition
statement is a kind of **manifestation statement** (LRM-E4-A4, examples "4th revised ed."
in ISBD and "4th revised edition" in RDA conventions) [1]. So in library terms an "edition"
as a reader means it ("the Collector's Edition", "the paperback") is a manifestation; a
"revised edition" or a translation is also a new expression.

### 2.2 RDA and MARC 21

RDA (the cataloguing rules built on the LRM) records the edition on the manifestation. The
RDA Registry [4] defines `rdam:P30107` "has edition statement": "Relates a manifestation to
a statement that identifies an edition to which a manifestation belongs", `rdam:P30133`
"has designation of edition": "a word, character, or group of words or characters, that
identifies an edition to which a manifestation belongs", and `rdam:P30182` "has extent of
manifestation" (where page counts live). The RDA Toolkit text itself is a subscription
product and was not read [3].

MARC 21 field 250 "Edition Statement" [5]: "Information relating to the edition of a work as
determined by applicable cataloging rules". `$a` the edition statement (e.g. "2nd ed.",
"Third edition.", "Canadian edition."), `$b` the remainder (responsibility, parallel
statement, e.g. "4th ed. / $b revised by ..., Repr. with corrections."). Repeatable since
2013. A catalogue record is one manifestation; 250 is transcribed from the book. Open
Library's `edition_name` (section 4.1) plays the same role.

### 2.3 Same edition or a new one: the cases

Combining LRM [1], the ISBN manual [6] and the trackers:

| Change | LRM | ISBN rule [6] | Reader's view |
|---|---|---|---|
| New printing (impression, reprint), unchanged | same manifestation | no new ISBN ("unchanged reprint") | same edition |
| New cover design or colour, new price | same manifestation (minor packaging) | "does not require a separate ISBN" (may get one for a media tie-in) | same edition, other cover |
| Hardcover vs paperback vs ebook format | new manifestation | new ISBN per product form and per file format | different edition |
| Audiobook (recording) | new expression (spoken word) and new manifestation | eligible for ISBN; own ISBN; CD vs download differ | different edition, different unit (minutes) |
| Other publisher or imprint | may be new manifestation | new ISBN | different edition |
| Translation | new expression | new ISBN ("each different language edition") | different edition, other language and title |
| Revised or expanded text, abridgement | new expression | new ISBN ("textual content other than minor corrections") | different edition; heavy rewrites are new works |
| Added illustrations, maps, notes | aggregate with other expressions; new manifestation | usually new ISBN (new content and form) | different edition ("illustrated edition") |
| Special or collector's edition (new layout, extras) | new manifestation | new ISBN | different edition (OT-26) |
| Adaptation for children, graphic novel, summary | new work | new ISBN | another book |

## 3. ISBN rules (International ISBN Agency, Users' Manual 7th ed., 2017)

The English manual is published as a Word file [6]; its text was extracted and read in full.

- Scope: the ISBN identifies "one publication or edition of a publication published by one
  specific publisher in one specific format". "Abstract entities such as textual works" are
  not eligible. So an ISBN identifies a manifestation, never a work.
- Definitions: **edition** = "all manifestations of a publication embodying essentially the
  same content"; "A single edition of a publication may be published in multiple product
  forms"; "Significant updates, revisions or changes to the content constitute a new edition
  and are usually numbered on the title page". **Impression** = "a print run of a given
  edition". **Reprint** = "A second or subsequent print run (impression) of the same edition".
  **Product form** examples: "hardcover, paperback; CD audio-book; Braille; online e-book".
  Note: the ISBN manual's "edition" is content-level (close to the LRM expression), while a
  reader's "edition" is usually the product (manifestation). The ISBN follows the product.
- Section 6.1: "A separate ISBN shall be assigned to each separate monographic publication or
  separate edition or format", "to each different language edition", and to each product
  form, product form detail, product form feature and set of usage constraints. "A separate
  ISBN should not be assigned to an unchanged edition or unchanged reprint ... in the same
  product form ... by the same publisher or imprint."
- Section 6.2, a new ISBN is required for changes to: title or subtitle, language, publisher
  name or imprint, author name, "textual content other than minor corrections to misprints",
  product form ("from hardback to paperback; from audiobook on a physical carrier to
  audiobook available to download"), product form detail ("from .pdf to EPUB; change of book
  dimension"), product form features, usage constraints. "A change to the cover design or
  colour or to the price ... does not require a separate ISBN".
- Ebooks: "each different format that is published and made separately available should be
  given a separate ISBN" (EPUB, PDF, platform-bound formats). Example of one title:
  hardback, paperback, PDF and EPUB, four ISBNs.
- Audiobooks: eligible, "either on a physical carrier or accessible on the Internet for
  download or streaming".
- Print on demand: a new ISBN only when the product form changes; minor trim changes need
  none.
- Facsimile reprint by another publisher, or under another imprint: new ISBN.
- Multi-volume sets: one ISBN for the set, one per volume sold separately.
- Section 7.8: "an ISBN can never be reused", even when assigned in error. In practice errors
  exist: StoryGraph tells users to leave the ISBN blank "If an ISBN has been recycled" [24].
- ISBN-10 vs ISBN-13: "Since 1 January 2007, national ISBN registration agencies only provide
  ISBNs that consist of 13 digits" [6][7]. The prefix "can only be either 978 or 979" [7].
  The 13-digit check digit is modulus 10 with weights 1 and 3 [6][7]; the manual points to
  the agency website for the 10-digit method [6]. Conversion as practised: "978" + the first
  nine digits of the ISBN-10 + a new check digit. Verified on the OT-26 book: `/isbn/1547604174`
  and `/isbn/9781547604173` both lead to edition OL28352792M [12] (this also answers the open
  point of DEC-719: Open Library finds an edition by either form). Inference, not stated in
  the read pages: a 979 ISBN has no ISBN-10, because an ISBN-10 carries no prefix and maps to
  978 only.
- History: the numbering started in the UK in 1967 (Whitaker) and the US in 1968, ISO 2108
  in 1970 [6]. This supports the 1967 cut-off of DEC-859 (an edition with an ISBN cannot be
  older).
- Books without an ISBN: old books (before 1967/1970), many small and private printings, and
  personal or customised print-on-demand books ("shall not be assigned an ISBN"). The ISBN is
  not a legal duty everywhere: "in some countries the use of ISBN ... has been made into a
  legal requirement" [7]. Self-publishers may get ISBNs (section 12). Libib warns: "Books
  published prior to 2005 may not have an ISBN barcode on the back cover" and points to
  keyword search or manual entry [34]. So an edition must be possible without an ISBN.

## 4. How the sources model editions

### 4.1 Open Library

Model [8]: "A Work is a logical collection of similar Editions." An edition "will have a
publisher, an ISBN, a book-jacket, and other specific information". Ids: work `OL...W`,
edition `OL...M`.

Endpoints:

- `GET /works/{id}/editions.json` [8][12]. The docs name the endpoint only; paging was read
  from live answers: `{"links": {"self", "work", "next"}, "size": <total>, "entries": [...]}`;
  `limit` and `offset` work; `links.next` is present while more pages exist (e.g.
  `?limit=2&offset=2`). `limit=1000` returned all 482 editions of The Hobbit (OL27482W) in
  one answer; a cap above that was not tested. A wrong work id answers `{"detail":"Not Found"}`.
  The order of entries is not documented. 1.0.1 already reads this list with `limit=100`
  for the year rule (DEC-859).
- `GET /isbn/{isbn}.json`: a 302 to `/books/{OLID}.json` [8][12]; ISBN-10 and ISBN-13 both work.
- `GET /books/{OLID}.json`: one edition.
- `search.json` with `fields=editions,editions.key,...` [9]: "The `editions` sub-object
  contains the editions of this work that match the user's query"; "Currently only one
  edition is displayed"; `editions.sort` changes the choice; `lang` "influences but doesn't
  exclude search results"; `language:fre` in the query excludes works without a French
  edition. Live: `q=isbn:9781547604173` answered the work OL17352669W with exactly the edition
  OL28352792M and its `language`, `publisher`, `format`, `isbn` [12]. So a search lists works,
  each with at most one matching edition; the full list needs `editions.json`.

Edition fields. The Books API page defines only some fields (publishers, publish_places,
publish_date "as a string", number_of_pages "Number of pages in that book", identifiers,
cover); `physical_format`, `pagination`, `languages`, `covers` appear only in examples;
`edition_name` and `translation_of` are not documented [8]. The search docs warn "The
schema is not guaranteed to be stable" [9]. Live coverage [12]:

| Field | A Court of Thorns and Roses (38 editions) | The Hobbit (482 editions) | Notes |
|---|---|---|---|
| `publishers` | 38 | n/c | list of names, free text ("Bloomsbury", "Bloomsbury YA", "Bloomsbury Publishing USA") |
| `publish_date` | 37 | 474 | free text: "2020", "Nov 05, 2019", "2022-04-13", "05/05/2015", "22. Mai 2020", "11 dezembro 2015" |
| `physical_format` | 33 | 299 | free text in many languages: "hardcover", "Hardcover", "Paperback", "Taschenbuch", "gebundene Ausgabe", "Flexibound", "library binding", "eBook", "audio cd", "Audiobook", "audible audiobook", "[sound recording] /"; Hobbit: 183 without |
| `number_of_pages` | 31 | 294 | integer; audiobooks hold 1 or 16 (disc counts) |
| `pagination` | 9 | 178 | catalogue extent: "433 pages : map, black and white illustrations", "419 pages : map", "454, [1] p", "454", "13 audio discs (16 hr.)" |
| `edition_name` | 10 | 137 | "Collector's Edition", "First Edition", "Unabridged", "Edition collector", "Edicao especial", "Wydanie I" |
| `languages` | 35 | 345 | keys like `/languages/eng`, `/languages/ger` (MARC codes, 3 letters) |
| `isbn_13` / `isbn_10` | 37 / 32 | 336 / 340 | lists |
| `translation_of` | 18 | 77 | the original title as a string ("A Court of Thorns and Roses"), not a key; `translated_from` (18) holds language keys |
| `covers` | 38 | 320 | cover ids per edition (`-1` entries occur) |
| `title`, `subtitle`, `full_title` | 38, 7, 20 | n/c | edition title, in the edition's language (Hobbit first entry: "Hobit ali Tja in spet nazaj") |
| `works` | 38 | n/c | the link back to the work |

(n/c = not counted.) Other fields seen: `series`, `publish_places`, `oclc_numbers`, `lccn`,
`physical_dimensions`, `weight`, `source_records` (e.g. `amazon:`, `bwb:` bulk imports, the
origin of the OT-36 bad dates).

The OT-26 edition, live [12]: `/isbn/9781547604173` -> OL28352792M: publishers "Bloomsbury
YA", physical_format "hardcover", edition_name "Collector's Edition", publish_date "Nov 05,
2019", number_of_pages 448, pagination "433 pages : map, black and white illustrations",
languages eng, covers [10300042] (its own cover), work OL17352669W. The regular 2015 Bloomsbury
hardcover has 432 pages, pagination "419 pages : map". Audio editions of the same work: Recorded
Books "Unabridged" (CD, `number_of_pages` 1; and "13 audio discs (16 hr.)"), Audible, Graphic
Audio; no duration field anywhere.

What Hordwyn fetches today (src/rust/crates/connectors/src/openlibrary.rs,
docs/connectors/openlibrary.md): search rows `key,title,author_name,first_publish_year,
cover_i,edition_count,number_of_pages_median`; an ISBN query asks `/isbn/{isbn}.json` and keeps
only the edition's `title` and `works[0].key`, then returns the **work** with the external id
`("isbn", isbn13)` and the attribute `isbn13`; `/works/{id}.json` for title, description,
subjects, covers; `/works/{id}/editions.json?limit=100` reading only `publish_date`,
`isbn_13`, `isbn_10` for `plausible_year` (DEC-859). The page total is
`number_of_pages_median` with provenance `openlibrary`, shown as "(estimate)" (DEC-716); a
person's total has provenance `user` and survives every refresh (DEC-715). Policy: one
request per second shared across processes (DEC-729), User-Agent with operator contact when
set (DEC-714), responses cached 7 days. Audiobooks are not served (docs/connectors/openlibrary.md:
"the documented fields hold no length and no narrator").

### 4.2 Wikidata

Model [13][14][17]: `Q7725634` "literary work" ("written work read for enjoyment or
edification"); WikiProject Books uses `Q47461344` "written work" (and subclasses) for works.
`Q3331189` "version, edition or translation": "specific expression or manifestation of a
work, consisting in its adaptation, edition, or translation". WikiProject Books "collapses
FRBR's expression and manifestation into edition": two levels, work and edition [14]. Links:
`P747` "has version, edition or translation" on the work, `P629` "version, edition or
translation of" on the edition (inverse pair). Edition properties: `P212` ISBN-13, `P957`
ISBN-10, `P1104` number of pages, `P123` publisher, `P577` publication date, `P407` language
of work or name, `P393` edition number, `P291` place of publication, `P655` translator,
`P1476` title. `P437` "distribution format" ("method (or type) of distribution") appears in
the project page only as a qualifier [14][17]. Guidance: a new edition item when content,
pagination, publisher or place changes; identical reprints need none [14]. The project page
was read to 100,000 of 194,356 characters.

How sparse (live SPARQL, 2026-11-20) [17]:

- 555,143 items are direct instances of `Q7725634`; 72,460 of them (about 13 percent) have at
  least one item pointing at them with `P629`.
- 861,914 items are instances of `Q3331189`; 319,365 of them carry an ISBN-13 (`P212`). A count
  of those with `P1104` timed out.
- The OT-26 work (`Q101987009`, "2015 novel by Sarah J. Maas") has **no** edition items, no
  ISBN and no page count; it has the Open Library work id (`P648` OL17352669W) and `P577` 2015.
- The Hobbit (`Q74287`): 19 edition items, 11 ISBN-13 values, 13 page values; Open Library has
  482 editions. Pride and Prejudice (`Q170583`): 34 edition items.

So Wikidata is a good bridge (work id to Open Library work id) and a second year source, but
rarely has the edition a reader holds. Etiquette: descriptive User-Agent with contact [16].

### 4.3 BookBrainz

Model [18]: **Work** "A distinct intellectual or artistic creation ... It is not the physical
book"; **Edition** "A published physical or digital version of one or more Works, such as a
book, anthology, or magazine"; **Edition Group** "a logical grouping of different Editions of
the same book" (paperback, hardcover and ebook of one book); Author (incl. translators,
illustrators), Publisher, Series. So BookBrainz has three levels: work (content), edition
group (the book as sold in many forms) and edition (one product), with works linked to
editions (an anthology holds many works).

API [20]: `https://api.bookbrainz.org/1/`, Swagger docs (the page is script-rendered; the
spec was not readable by the fetcher). Live: `/1/search?q=Ancillary%20Mercy&type=edition`
answered 14 editions; `/1/edition/{bbid}` answered `editionFormat` ("eBook"), `pages` (368),
`languages`, `publishers`, `releaseEventDate`, `status`, `width/height/depth/weight`,
`authorCredits`, `defaultAlias`. A search for an ISBN as `q` answered nothing; an ISBN lookup
path was not found in the readable docs. "A Court of Thorns and Roses": 10 edition hits, 68
work hits (by name). Site is "Alpha Software"; coverage is small. Data CC0, code GPL-2.0 [19][21].

### 4.4 How trackers present editions to readers

**Goodreads** [22][23] (help pages not fetchable; from search summaries of help questions and
Librarians Group threads, 2013-2024). A book page is one edition; "other editions" / "all
editions" lists the rest; each has "Switch to this edition". Switching moves the shelf entry
and the review to the new edition; several editions can be shelved at once if added from the
book page instead of switched. Users report: switching is desktop only (not in the Android
app), and a Currently Reading book could not be switched, so they removed and re-added it,
which "fixed their page-count tracking" but changed shelving dates. Moderators advise
searching by ISBN or ASIN to shelve the exact edition. Librarians "combine" editions under one
work and "don't move ISBNs or ASINs between editions".

**StoryGraph** [24][25][26]. A book shows "the number of editions"; the editions page can be
scrolled, filtered or searched by ISBN; "Switch to this edition" (in the app behind the
three-dot menu). Missing editions are added by users ("Add Edition": ISBN, page count, cover).
Audio: since 2022 the default is to count audiobooks as pages ("we'll automatically convert
everything to pages for you"), or a person switches to minutes ("Convert all data to
minutes"), with hours goals. Users who switch format mid-read must mark the old edition
DNF or lose credit, which skews DNF stats; a roadmap reply says to "switch editions" at each
format change and then "mark as finish" [26].

**Hardcover** [28]-[33]. A **book** is the work; an **edition** "represents a specific
published version" with `edition_format`, `reading_format_id` (1 Physical, 2 Audio, 3 Both,
4 Ebook), `pages`, `audio_seconds`, `isbn_10`, `isbn_13`, `asin`, `publisher`, `release_date`,
`language`, `edition_information`, its own `image`, and `source` ("OpenLibrary, manual, etc.").
Each book has a default edition per format: `default_physical_edition`,
`default_ebook_edition`, `default_audio_edition`. The library entry (`user_books`) has an
optional `edition_id`; each **read** (`user_book_reads`, one per pass, re-reads keep their own
dates) has its own edition and `progress_pages`, or `progress_seconds` for an audiobook;
"To show a percentage, work it out from the edition's pages or audio_seconds". "Progress
that reaches the end of the edition finishes the read". Librarian page-count standard: "Use the
page count listed in the edition itself, typically found on the copyright page or the last
numbered page of the book ... avoid including blank and unnumbered pages"; ebooks prefer
print-referenced page numbers [33]. This is the closest model to the owner's wish.

**Libib** [34]. Add by "UPC/ISBN" ("immediate lookup and add", the most accurate), by
keywords, by LCCN, by manual entry or CSV. A scanned ISBN is the item; the help does not
describe choosing among editions. A cataloguing tool, not a reading tracker (no progress).

**Calibre** [36]. One library record per book; files of different formats (EPUB, PDF, ...)
are formats of that one record ("All e-book files in a folder are assumed to be the same book
in different formats"); identifiers such as `isbn:` and a publisher and published date belong
to the record; "Merge book records" combines records. No edition concept; no progress.

Summary of reader flows:

| Tracker | Unit in the library | Edition required | Pick when adding | Switch later | Progress on switch | Audio |
|---|---|---|---|---|---|---|
| Goodreads | edition | yes (some edition) | search result is an edition; ISBN finds the exact one | "Switch to this edition" (desktop) | reported trouble while Currently Reading | an edition like others |
| StoryGraph | edition | yes | editions page with filter and ISBN search | "Switch to this edition" | users report it is kept; format switch mid-read is awkward | edition with minutes; pages or minutes setting |
| Hardcover | book (work) + optional edition | no | default edition per format | edition per entry and per read | per read: pages or seconds of that read's edition | edition with `audio_seconds` |
| Libib | scanned item | n/a | ISBN | no | n/a | n/a |
| Calibre | record with formats | no | n/a | n/a | n/a | n/a |

## 5. What an edition changes for a reader

**Page count and pagination.** Totals differ by edition: the OT-26 work has editions from
392 to 560 pages (translations included) and the median 454 is none of the English ones [12].
Open Library has two page fields: `number_of_pages` (an integer, often the full count) and
`pagination` (the catalogue extent statement, present on 9 of 38 and 178 of 482 editions).
For the OT-26 copy the numbered pages are 433 against 448 in `number_of_pages`; the regular
edition 419 against 432. Hardcover's standard also uses the last numbered page and leaves out
blank and unnumbered pages [33]. The `pagination` text needs a parser: leading number before
"pages", "p", "p." or a comma; "[1]" marks pages without numbers (library practice, not
verified against a source in this research); roman preliminaries ("xii, 433 p.") occur in
catalogue records generally; audio extents ("13 audio discs (16 hr.)") must not be read as
pages. Audio editions put disc counts into `number_of_pages` (1, 16), so any page figure from
an audio edition is wrong for reading. Whether Open Library leaves audio editions out of
`number_of_pages_median` is not documented (unverified). Hordwyn's progress model counts pages
and keeps a total set by a person with provenance `user` (DEC-715, DEC-716); a change of
edition changes the total, so progress stored as a page number can exceed or fall short of the
new total; percent is the measure that survives a change (Hardcover computes percent from the
read's edition [31]; StoryGraph users log "that same percentage" after a switch, per search
summary of [26] and related posts).

**Audiobook vs print.** By LRM an audiobook is a new expression (spoken word) of the same work
and a new manifestation [1]; by ISBN rules an audiobook is a product form of the edition with
its own ISBN [6]; Wikidata, BookBrainz, Hardcover and StoryGraph all treat it as an edition of
the same book [14][18][29][25]. Its length is time, not pages (Hardcover `audio_seconds`,
StoryGraph minutes). Open Library has audio editions but no duration [12]. Hordwyn has a
separate `audiobook` kind (minutes) and no audiobook source.

**Language and title.** The edition's language and title differ from the work's: Open Library
editions carry `languages` and their own `title` ("Hobit ali Tja in spet nazaj"),
`translation_of` holds the original title [12]; a translation is a new expression [1] and a
new ISBN [6]. This touches OT-34 part 2 (the stored item is fetched in the instance
language): an edition gives a title in the language of the copy the person reads, without a
per-language title store for the work.

**Cover.** Covers are per edition at Open Library (`covers` on each edition; the Collector's
Edition has its own) [12], per edition at Hardcover [29] and StoryGraph [24]. A new cover alone
does not make a new ISBN [6], so two covers may share one ISBN.

**Year.** Two years: the work's first publication and this edition's date. The OT-26 copy is
2019, the work 2015. Open Library `publish_date` is free text in many formats and languages
[12]; `first_publish_year` and the vote of DEC-859 give the work's year. ISBNs started in
1967/1970 [6], the base of DEC-859's cut-off. Hardcover keeps `release_date` per edition and
per book [29][32].

**The owner's cases** (OT-26, v2-notes). (a) An ISBN identifies exactly one manifestation [6],
and Open Library resolves both ISBN forms to the edition [12]: an ISBN search can show that
edition and its pages without estimate. (b) A title search answers works with at most one
matching edition each [9]; the list to pick from needs `editions.json` per work (one request;
38 for the OT-26 work, 482 for The Hobbit) [12]; filtering by `edition_name`,
`physical_format`, publisher or language is possible on that list, but the values are free
text in many languages. (c) Numbered pages come from `pagination` where present (9 of 38),
else `number_of_pages`. (d) "The edition is optional and changeable on the entry": Hardcover
does exactly this (optional `edition_id` on the library entry, an edition per read) [30][31];
Goodreads and StoryGraph force some edition and offer "switch" [23][24]. Worked example: the
go file asks to use the book the owner started on 2026-11-20; this research could not ask the
owner, so the OT-26 book (ISBN 978-1-5476-0417-3) is used here and the owner's book should be
added during the design review.

## 6. Findings that matter for the design

1. An "edition" as a reader means it is the LRM **manifestation** (format, publisher, layout,
   ISBN); revisions, abridgements and translations are also new expressions. No model has an
   entity named edition; the edition statement is a manifestation attribute [1][2][4][5].
2. The ISBN identifies one product (manifestation), never the work: a new ISBN per format,
   ebook file format, language, publisher or imprint, and content change; none for reprints,
   new covers or prices [6]. An ISBN hit is therefore the edition, as the owner wants (OT-26).
3. An edition cannot require an ISBN: books before 1967/1970, private and personal printings
   have none; ISBNs can be wrong or recycled in practice [6][7][24][34].
4. Open Library resolves ISBN-10 and ISBN-13 to the same edition record (`/isbn/` 302 to
   `/books/OL...M`) [12]; the 13-digit check digit is mod 10 with weights 1 and 3 [6][7].
5. Open Library edition records hold what an edition needs: `edition_name`, `physical_format`,
   `publishers`, `publish_date`, `number_of_pages`, `pagination`, `languages`, ISBNs, `covers`,
   `title`, `translation_of`. Most are undocumented and free text: formats and dates come in
   many languages and spellings; coverage is partial (`pagination` 9/38 and 178/482,
   `edition_name` 10/38 and 137/482) [8][9][12].
6. `editions.json` pages with `limit`/`offset`, `size` and `links.next`; `limit=1000` answered
   482 entries at once [12]. One request per work fits the 1 request/s policy; search.json
   gives only one matching edition per work [9][10].
7. Numbered pages: `pagination` gives the readable pages (OT-26: 433 vs 448; regular: 419 vs
   432), matching Hardcover's librarian rule (last numbered page) [12][33]. A parser must
   handle "N pages : ...", "N p.", "N, [1] p", roman preliminaries and audio extents.
8. Audio editions sit in the same Open Library work with disc counts in `number_of_pages` (1,
   16) and no duration; they must not feed page totals [12]. LRM makes an audiobook a new
   expression [1]; ISBN, Wikidata, BookBrainz, Hardcover and StoryGraph treat it as an edition
   of the same book with time as its length [6][14][18][29][25].
9. Wikidata is too sparse for editions: about 13 percent of literary works have any edition
   item; the OT-26 work has none (but carries the Open Library work id) [17]. Useful as a
   cross-check for the work year (OT-36), not as the edition source. CC0; needs a descriptive
   User-Agent with contact [15][16].
10. BookBrainz has the cleanest model (work / edition group / edition, CC0 core data) but is
    alpha with small coverage and no readable ISBN lookup; code is GPL-2.0 (never copy)
    [18][19][20][21].
11. The tracker closest to the owner's wish is Hardcover: work in the library, edition optional
    on the entry and set per read, progress in pages or seconds of that read's edition, percent
    derived, default edition per format [29][30][31][32]. Goodreads and StoryGraph tie the entry
    to an edition and offer "Switch to this edition"; their users report lost dates, DNF
    workarounds and page-count trouble when switching mid-read [23][24][26].
12. Progress stored as pages is edition-bound; on an edition change the page number means
    something else. The trackers either keep per-read editions (Hardcover) or let users re-log
    the same percentage (StoryGraph users) [26][31].
13. Edition title, language and cover belong to the edition [12][29]; the edition's title in
    the reader's language is a partial answer to OT-34 part 2.
14. Two years exist: first publication (work) and this edition's date; Open Library's
    `publish_date` is free text and bulk-imported records carry wrong dates (OT-36,
    `source_records` amazon/bwb) [12]; the 1967 rule of DEC-859 is backed by ISBN history [6].
15. Licences: Open Library (no rights asserted, AGPL software: docs only), Wikidata (CC0) and
    BookBrainz core data (CC0) may be used; Goodreads (no new API keys since 2020), StoryGraph
    (no API), Libib (no scraping) are not data sources; Hardcover's beta API needs a per-user
    token and forbids using other users' data; Calibre is GPL-3.0 and read only [10][11][15]
    [19][22][27][28][35][37].

## References (all read 2026-11-20)

1. IFLA Library Reference Model (LRM), August 2017, rev. December 2017, PDF:
   https://repository.ifla.org/bitstreams/95db0627-0a52-466f-a7d0-6842b6012838/download
   (item page: https://repository.ifla.org/items/6ffe1434-b660-49e9-875f-df90b9ff4e54/full).
   The IFLA website page on the FRBR family answered 403.
2. Functional Requirements for Bibliographic Records, Final Report (1997, amended 2009):
   https://www.ifla.org/files/assets/cataloguing/frbr/frbr_2008.pdf
3. RDA Toolkit, About: https://www.rdatoolkit.org/about (Toolkit text not read: subscription)
4. RDA Registry: https://www.rdaregistry.info/ ; manifestation elements (JSON-LD):
   https://www.rdaregistry.info/jsonld/Elements/m.jsonld (P30107, P30133, P30182). The HTML
   element page https://www.rdaregistry.info/Elements/m/P30133 answered 404.
5. MARC 21 Bibliographic, 250 Edition Statement: https://www.loc.gov/marc/bibliographic/bd250.html
6. International ISBN Agency, ISBN Users' Manual, International Edition, Seventh Edition
   (2017), Word file:
   https://www.isbn-international.org/sites/default/files/ISBN%20International%20Users%20Manual%20-%207th%20edition_absolutely_final.docx
   (listed on https://www.isbn-international.org/content/isbn-users-manual/29). A PDF mirror
   could not be read as text.
7. International ISBN Agency, What is an ISBN?: https://www.isbn-international.org/content/what-isbn/10
8. Open Library, Books API (works, editions, ISBN): https://openlibrary.org/dev/docs/api/books
9. Open Library, Search API: https://openlibrary.org/dev/docs/api/search
10. Open Library, Developers API (rate limits, User-Agent, usage guidelines):
    https://openlibrary.org/developers/api
11. Open Library, Licensing: https://openlibrary.org/developers/licensing
12. Open Library live answers: https://openlibrary.org/works/OL17352669W/editions.json?limit=2 ,
    https://openlibrary.org/works/OL17352669W/editions.json?limit=50 ,
    https://openlibrary.org/works/OL27482W/editions.json?limit=1000 ,
    https://openlibrary.org/works/OL45804W/editions.json?limit=1000 ,
    https://openlibrary.org/isbn/9781547604173.json , https://openlibrary.org/isbn/1547604174.json ,
    https://openlibrary.org/search.json?q=isbn:9781547604173&fields=key,title,editions,editions.key,editions.title,editions.language,editions.publisher,editions.isbn,editions.format&limit=1
13. Wikidata, Q3331189: https://www.wikidata.org/wiki/Q3331189
14. Wikidata, WikiProject Books: https://www.wikidata.org/wiki/Wikidata:WikiProject_Books
    (read to character 100,000 of 194,356)
15. Wikidata, Licensing: https://www.wikidata.org/wiki/Wikidata:Licensing
16. Wikimedia Foundation User-Agent Policy:
    https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy
17. Wikidata live: https://query.wikidata.org/sparql (counts of Q7725634, Q3331189, P212,
    P629; editions of Q74287, Q170583, Q101987009);
    https://www.wikidata.org/wiki/Special:EntityData/Q101987009.json ,
    https://www.wikidata.org/wiki/Special:EntityData/Q7725634.json ,
    https://www.wikidata.org/wiki/Special:EntityData/P437.json ,
    https://www.wikidata.org/wiki/Special:EntityData/P629.json ,
    https://www.wikidata.org/wiki/Special:EntityData/P747.json
18. BookBrainz, Help: https://bookbrainz.org/help
19. BookBrainz, Licensing: https://bookbrainz.org/licensing ; About: https://bookbrainz.org/about
20. BookBrainz API: https://api.bookbrainz.org/1/api-docs/ (script-rendered, spec not
    readable); live: https://api.bookbrainz.org/1/search?q=Ancillary%20Mercy&type=edition&size=3 ,
    https://api.bookbrainz.org/1/edition/3fa9fdcd-098d-4ec1-82e4-f5fdfb92c41f
21. bookbrainz-site repository (licence): https://github.com/metabrainz/bookbrainz-site
22. Goodreads Developers, API deprecation (via search summary; the API page
    https://www.goodreads.com/api answered empty and the help article
    https://help.goodreads.com/s/article/Does-Goodreads-support-the-use-of-APIs could not be
    rendered): https://www.goodreads.com/topic/show/21788520-api-deprecation
23. Goodreads help questions and Librarians Group threads (not renderable; via search
    summaries): https://help.goodreads.com/s/question/0D58V00006PfRDbSAN/how-do-i-change-the-edition-of-a-book-im-currently-reading ,
    https://help.goodreads.com/s/question/0D58V000071pABRSA2/how-do-i-switch-book-editions-on-the-new-book-page ,
    https://www.goodreads.com/topic/show/18427635-possible-to-change-edition-after-a-book-is-read ,
    https://www.goodreads.com/topic/show/22770692-how-to-shelve-multiple-editions-of-a-book
24. StoryGraph, Switching editions/formats:
    https://thestorygraph.freshdesk.com/support/solutions/articles/79000141927-switching-editions-formats-on-the-storygraph
25. StoryGraph, Audiobook tracking and stats: https://roadmap.thestorygraph.com/changelog/audiobook-tracking-and-stats-
26. StoryGraph roadmap, DNF stats skewed by edition swapping:
    https://roadmap.thestorygraph.com/requests-ideas/posts/dnf-stats-skewed-by-edition-swapping
27. StoryGraph roadmap, An API (via search summary): https://roadmap.thestorygraph.com/features/posts/an-api ;
    terms https://www.thestorygraph.com/terms answered 403
28. Hardcover, API Getting Started: https://docs.hardcover.app/api/getting-started/
    (the policies page https://hardcover.app/pages/policies was not read)
29. Hardcover, Editions schema: https://docs.hardcover.app/api/graphql/schemas/editions/
30. Hardcover, User Books schema: https://docs.hardcover.app/api/graphql/schemas/userbooks/
31. Hardcover, Updating a Book's Progress and Dates Read:
    https://docs.hardcover.app/api/guides/updatingabooksprogress/
32. Hardcover, Getting Book Details: https://docs.hardcover.app/api/guides/gettingbookdetails/
33. Hardcover, Edition Standards: https://docs.hardcover.app/librarians/standards/editionstandards/
34. Libib, Add items: https://support.libib.com/libib/website/add-items.html
35. Libib, Terms of Service: https://www.libib.com/terms-of-service
36. calibre User Manual, The Graphical User Interface: https://manual.calibre-ebook.com/gui.html
37. calibre repository (licence): https://github.com/kovidgoyal/calibre

Project files used: docs/handoff/1.1.0-go.md, docs/handoff/owner-test-2026-11-15.md,
docs/handoff/v2-notes.md, docs/DECISIONS.md (DEC-711..732, DEC-733..758, DEC-859),
docs/connectors/openlibrary.md, src/rust/crates/connectors/src/openlibrary.rs.
