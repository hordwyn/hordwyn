//! `cargo xtask upgrade-test [<previous-tag>]` (v1-release item 6): which release it upgrades
//! from, and that CI runs it in every pipeline.

use xtask::upgrade::{is_release_tag, previous_release};

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn release_tags_are_v_and_a_number() {
    for t in ["v1", "v1.0.0", "v2.3.1", "v10"] {
        assert!(is_release_tag(t), "{t}");
    }
    for t in [
        "m08",
        "v",
        "vnext",
        "1.0.0",
        "release-1",
        "",
        "v1-polish",
        "v1.",
        "v1..2",
        "v1.0.0-rc.1",
        "v1.0.0-rc1",
    ] {
        assert!(!is_release_tag(t), "{t}");
    }
}

/// Before v1 there is no release to upgrade from (the milestone tags `m<NN>` are not
/// releases): the command skips. On a release tag the previous one is used, not the tag of
/// the commit itself; between tags the newest one.
#[test]
fn the_previous_release_is_the_newest_tag_below_this_build() {
    assert_eq!(previous_release(&tags(&["m13", "m08"]), &[]), None);
    assert_eq!(previous_release(&tags(&[]), &[]), None);
    // on the v1 tag itself: nothing before it
    assert_eq!(
        previous_release(&tags(&["v1.0.0"]), &tags(&["v1.0.0"])),
        None
    );
    // newest first, as `git tag --merged HEAD --sort=-v:refname` lists them
    let merged = tags(&["v1.1.0", "v1.0.1", "v1.0.0", "m13"]);
    assert_eq!(
        previous_release(&merged, &tags(&["v1.1.0"])),
        Some("v1.0.1".into())
    );
    assert_eq!(previous_release(&merged, &[]), Some("v1.1.0".into()));
    // a release candidate is not a release to upgrade from
    assert_eq!(
        previous_release(&tags(&["v1.1.0-rc.1", "v1.0.0"]), &[]),
        Some("v1.0.0".into())
    );
}

/// REL-5: the job runs in every pipeline (main, merge requests, tags), not only on the release
/// tag: a changed released migration is found before a tag is pushed. Before v1 it skips.
#[test]
fn ci_runs_the_upgrade_test_in_every_pipeline() {
    let ci = std::fs::read_to_string(xtask::repo_root().join(".gitlab-ci.yml")).unwrap();
    let job = ci
        .split("\nupgrade-test:\n")
        .nth(1)
        .expect("a job `upgrade-test` in .gitlab-ci.yml");
    let job: String = job
        .lines()
        .take_while(|l| l.is_empty() || l.starts_with(' ') || l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !job.contains("rules:") && !job.contains("only:"),
        "the upgrade-test job is limited to some pipelines:\n{job}"
    );
    for needle in ["GIT_DEPTH: \"0\"", "cargo xtask upgrade-test", "postgres"] {
        assert!(
            job.contains(needle),
            "the upgrade-test job lacks `{needle}`"
        );
    }
}

/// The seed of this build is what the next release's upgrade test reads from this tag.
#[test]
fn the_seed_of_this_build_is_in_the_tree() {
    let seed = xtask::repo_root().join(xtask::upgrade::SEED);
    let text = std::fs::read_to_string(&seed).unwrap_or_else(|e| panic!("{}: {e}", seed.display()));
    assert!(text.contains("INSERT INTO"), "the seed inserts nothing");
    assert!(
        !text.contains("\\restrict"),
        "psql meta-commands do not run through sqlx"
    );
}

/// REL-3: `cargo xtask upgrade-seed` turns a `pg_dump --data-only --inserts` of the seed
/// database into the seed: no psql or session commands, the accounts first (their
/// self-companion key is deferred, the companions' owner key is not), the settings row
/// replaced instead of inserted twice, the trigger-written `stats_dirty` left out, the
/// sequences set, and the data itself untouched (a value that contains `public.`, a value
/// over several lines).
#[test]
fn a_dump_becomes_the_seed() {
    let dump = "\\restrict abc\nSET statement_timeout = 0;\n\
        SELECT pg_catalog.set_config('search_path', '', false);\n\
        -- Data for Name: companions\n\n\
        INSERT INTO public.companions (id, owner_id) VALUES ('c', 'u');\n\
        INSERT INTO public.instance_settings (id, mail_host) VALUES (true, 'm');\n\
        INSERT INTO public.stats_dirty (user_id) VALUES ('u');\n\
        INSERT INTO public.users (id, username) VALUES ('u', 'see public.example');\n\
        INSERT INTO public.reviews (id, body) VALUES ('r', 'two\nlines; it''s\n');\n\
        SELECT pg_catalog.setval('public.outbox_id_seq', 203, true);\n\
        \\unrestrict abc\n";
    let seed = xtask::upgrade::seed_from_dump(dump, "-- header\n");
    assert_eq!(
        seed,
        "-- header\nBEGIN;\n\
         DELETE FROM instance_settings;\n\
         INSERT INTO users (id, username) VALUES ('u', 'see public.example');\n\
         INSERT INTO companions (id, owner_id) VALUES ('c', 'u');\n\
         INSERT INTO instance_settings (id, mail_host) VALUES (true, 'm');\n\
         INSERT INTO reviews (id, body) VALUES ('r', 'two\nlines; it''s\n');\n\
         SELECT pg_catalog.setval('outbox_id_seq', 203, true);\n\
         COMMIT;\n"
    );
}
