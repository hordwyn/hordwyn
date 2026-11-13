# Application captures

Real captures of the application, running with invented sample data (2026-11-13): an
invented household, invented titles, no real person. No provider image is shown: the mock
catalogue serves none, every cover is the application's own placeholder.

Files: `<view>-<locale>-<theme>.png`, views `home`, `home-top` (1280x800), `item`,
`episodes`, `library`, `log` (390x780 at 2x), locales `en` and `de`, themes `light` and `dark`.

Regenerate (after `cargo leptos build` in `src/rust`, with the local Postgres running):

    cd src/web && nix develop ../.. -c node site-tests/capture/capture.mjs

The script and its options: `src/web/site-tests/capture/capture.mjs`.
