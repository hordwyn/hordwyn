//! Deployment files (package v1-ops): the image definition, the compose files and
//! `config/.env.example`, read as text (the build host has no container runtime).

use xtask::ops::{
    app_keys, commented_service, compose_problems, compose_services, dockerfile_instructions,
    dockerfile_problems, env_example_problems, section,
};

fn read(path: &str) -> String {
    let p = xtask::repo_root().join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

const OLD_IMAGE: &str = "FROM rust:1 AS build\n\
    RUN cargo leptos build --release --server-only \\\n    && cp target/release/server /out/server\n\
    FROM debian:trixie-slim AS runtime\n\
    COPY --from=build /out/server /usr/local/bin/server\n\
    ENV APP__STORAGE__FS_ROOT=/data \\\n    APP__SECRETS__KEY_FILE=/data/secrets.key\n\
    ENTRYPOINT [\"/usr/local/bin/server\"]\n";

#[test]
fn instructions_are_split_by_stage() {
    let ins = dockerfile_instructions(OLD_IMAGE);
    assert_eq!(ins.len(), 6);
    assert_eq!(ins[1].stage, "build");
    assert_eq!(
        ins[1].args,
        "cargo leptos build --release --server-only && cp target/release/server /out/server"
    );
    assert_eq!(ins[4].stage, "runtime");
    assert_eq!(ins[4].word, "ENV");
}

/// OPS-1, OPS-3: the checks name what the image before this package lacked.
#[test]
fn an_image_without_the_cli_or_with_the_key_in_the_blob_root_is_refused() {
    let found = dockerfile_problems(OLD_IMAGE).join("\n");
    assert!(found.contains("-p cli"), "{found}");
    assert!(found.contains("does not copy `cli`"), "{found}");
    assert!(found.contains("cli help"), "{found}");
    assert!(found.contains("inside the blob directory /data"), "{found}");
    assert!(!found.contains("does not copy `server`"), "{found}");
}

#[test]
fn the_image_contains_the_cli_and_keeps_the_key_out_of_the_blob_directory() {
    let found = dockerfile_problems(&read("src/docker/Dockerfile"));
    assert_eq!(found.len(), 0, "{}", found.join("\n"));
}

/// The image moves the blobs of an existing volume instead of starting on an empty store.
#[test]
fn the_image_names_the_earlier_blob_root() {
    let text = read("src/docker/Dockerfile");
    let ins = dockerfile_instructions(&text);
    assert!(
        ins.iter()
            .any(|i| i.word == "ENV" && i.args.contains("APP__STORAGE__FS_LEGACY_ROOT=/data")),
        "APP__STORAGE__FS_LEGACY_ROOT=/data is not set in the image"
    );
}

const COMPOSE_FILES: [&str; 4] = [
    "src/docker/compose.yml",
    "src/docker/compose.s3.yml",
    "src/docker/compose.podman.yml",
    "src/docker/compose.podman.s3.yml",
];

const OLD_COMPOSE: &str = "name: tracker\n\nservices:\n  app:\n    build:\n      context: .\n    environment:\n      \
    APP__DATABASE__URL: \"postgres://app:${POSTGRES_PASSWORD:-app}@postgres:5432/app\"\n    \
    ports:\n      - \"3000:3000\"\n    volumes:\n      - app-data:/data\n  init:\n    \
    image: x\n    restart: \"no\"\n\nvolumes:\n  app-data:\n";

/// SEC-6, OPS-7: the checks name what the compose file before this package did.
#[test]
fn defaults_for_credentials_and_open_ports_are_refused() {
    let services = compose_services(OLD_COMPOSE);
    assert_eq!(
        services.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["app", "init"]
    );
    let found = compose_problems(OLD_COMPOSE).join("\n");
    assert!(found.contains("${POSTGRES_PASSWORD:-app}"), "{found}");
    assert!(found.contains("port `3000:3000`"), "{found}");
    assert!(found.contains("service app has no healthcheck"), "{found}");
    assert!(found.contains("stop_grace_period"), "{found}");
    assert!(!found.contains("service init"), "{found}");
    let fine = "services:\n  app:\n    image: x\n    stop_grace_period: 30s\n    environment:\n      \
        A: \"${POSTGRES_PASSWORD:?set it}\"\n    ports:\n      \
        - \"${APP_PUBLISH_ADDR:-127.0.0.1}:3000:3000\"\n      - \"127.0.0.1:1:1\"\n    \
        healthcheck:\n      test: [\"CMD\", \"true\"]\n";
    assert_eq!(compose_problems(fine), Vec::<String>::new());
    let open = fine.replace(":-127.0.0.1", ":-0.0.0.0");
    assert_eq!(compose_problems(&open).len(), 1);
}

#[test]
fn compose_requires_its_credentials_and_publishes_on_loopback() {
    for file in COMPOSE_FILES {
        let text = read(file);
        let found = compose_problems(&text);
        assert_eq!(found.len(), 0, "{file}:\n{}", found.join("\n"));
    }
    let base = read(COMPOSE_FILES[0]);
    let services = compose_services(&base);
    let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["app", "postgres", "meilisearch"]);
    // the commented web/worker pair states the supported shape (OPS-12)
    assert!(base.contains("ONE web process plus any number of workers"));
    assert!(base.contains("#   command: [\"--role=worker\"]"));
}

#[test]
fn keys_named_in_a_text_are_found() {
    let keys =
        app_keys("set `APP__SERVER__PUBLIC_URL`, APP__AUTH__SECURE_COOKIES=false and APP__<X>.");
    assert_eq!(
        keys.into_iter().collect::<Vec<_>>(),
        vec!["APP__AUTH__SECURE_COOKIES", "APP__SERVER__PUBLIC_URL"]
    );
}

/// OPS-2: a missing first-run key, a compose variable or a key a page names is reported.
#[test]
fn a_missing_first_run_key_is_reported() {
    let reference =
        "`APP__SERVER__PUBLIC_URL` `APP__SERVER__TRUSTED_PROXIES` `APP__AUTH__SECURE_COOKIES`";
    let example =
        "# c\nPOSTGRES_PASSWORD=\nAPP__SERVER__PUBLIC_URL=\nAPP__AUTH__SECURE_COOKIES=true\n";
    let found = env_example_problems(
        example,
        &["x: ${POSTGRES_PASSWORD:?a} ${MEILI_MASTER_KEY:?b}\n# ${COMMENTED}"],
        &["set APP__SERVER__BIND"],
        reference,
    )
    .join("\n");
    assert!(
        found.contains("APP__SERVER__TRUSTED_PROXIES is not listed"),
        "{found}"
    );
    assert!(found.contains("MEILI_MASTER_KEY is not listed"), "{found}");
    assert!(found.contains("APP__SERVER__BIND is not listed"), "{found}");
    assert!(
        found.contains("APP__AUTH__SECURE_COOKIES has a value"),
        "{found}"
    );
    assert!(!found.contains("COMMENTED"), "{found}");
    assert!(!found.contains("POSTGRES_PASSWORD"), "{found}");
    let unknown = env_example_problems("", &[], &[], "").join("\n");
    assert!(
        unknown.contains("is not a setting of the configuration reference"),
        "{unknown}"
    );
}

#[test]
fn the_env_example_lists_every_first_run_key() {
    let found = env_example_problems(
        &read("config/.env.example"),
        &COMPOSE_FILES.map(read).each_ref().map(String::as_str),
        &[
            &read("docs/modules/ROOT/pages/install.adoc"),
            &read("docs/modules/ROOT/pages/reverse-proxy.adoc"),
        ],
        &read("docs/modules/ROOT/pages/configuration.adoc"),
    );
    assert_eq!(found.len(), 0, "{}", found.join("\n"));
}

/// The runbook is commands, not prose (OPS-9): each step an operator needs is there, with
/// the CLI called inside the container.
#[test]
fn the_runbook_names_its_commands() {
    let ops = read("docs/modules/ROOT/pages/operations.adoc");
    for needle in [
        "pg_dump -Fc",
        "pg_restore",
        "/data/secrets.key",
        "/data/blobs/up",
        "exec app cli reindex --clear",
        "cli app migrate --dry-run",
        "one web process",
    ] {
        assert!(ops.contains(needle), "operations.adoc lacks `{needle}`");
    }
    let nav = read("docs/modules/ROOT/nav.adoc");
    for page in ["install.adoc", "reverse-proxy.adoc"] {
        assert!(nav.contains(page), "nav.adoc lacks {page}");
    }
    let proxy = read("docs/modules/ROOT/pages/reverse-proxy.adoc");
    for needle in [
        "X-Forwarded-For",
        "Strict-Transport-Security",
        "X-Content-Type-Options",
        "/api/v1/events",
        "proxy_buffering off",
        "reverse_proxy",
    ] {
        assert!(
            proxy.contains(needle),
            "reverse-proxy.adoc lacks `{needle}`"
        );
    }
}

// ---------------------------------------------------------------- review 2026-11-12
// docs/reviews/2026-11-12-v1-ops-review.md. Text checks like the ones above: they prove what
// the pages and files say. No command of the runbook was run in a container on the build host.

const OPERATIONS: &str = "docs/modules/ROOT/pages/operations.adoc";

fn ops_section(title: &str) -> String {
    let page = read(OPERATIONS);
    section(&page, title)
        .unwrap_or_else(|| panic!("operations.adoc has no section `== {title}`"))
        .to_string()
}

fn expect_all(text: &str, what: &str, needles: &[&str]) {
    for needle in needles {
        assert!(text.contains(needle), "{what} lacks `{needle}`");
    }
}

/// `a` is written before `b`.
fn expect_order(text: &str, what: &str, a: &str, b: &str) {
    let (x, y) = (text.find(a), text.find(b));
    assert!(
        matches!((x, y), (Some(x), Some(y)) if x < y),
        "{what}: `{a}` must come before `{b}` ({x:?}, {y:?})"
    );
}

#[test]
fn a_section_ends_at_the_next_heading_of_its_level() {
    let page = "= P\n\n== One\na\n\n=== Deeper\nb\n\n== Two\nc\n";
    assert_eq!(section(page, "One"), Some("== One\na\n\n=== Deeper\nb\n\n"));
    assert_eq!(section(page, "Two"), Some("== Two\nc\n"));
    assert_eq!(section(page, "Deeper"), None);
    let compose = "services:\n  app:\n    image: x\n  # worker:\n  #   image: y\n  #   command: [\"a\"]\n\n  db:\n";
    assert_eq!(
        commented_service(compose, "worker"),
        vec!["  image: y", "  command: [\"a\"]"]
    );
    assert_eq!(commented_service(compose, "db").len(), 0);
}

/// V1OPS-1: an instance started with the compose file before this package (default
/// passwords, port 3000 on every interface, blobs in `/data`, no `config/.env` needed) gets
/// its own steps, in an order that works on the old checkout.
#[test]
fn the_upgrade_of_an_instance_from_before_0034_is_spelled_out() {
    let s = ops_section("Coming from a build before 0034");
    let what = "the section `Coming from a build before 0034`";
    expect_all(
        &s,
        what,
        &[
            // the environment file exists before any command reads it
            "cp config/.env.example config/.env",
            // the database keeps the password it was initialized with
            "POSTGRES_PASSWORD=app",
            "an existing database keeps its password",
            // ... or it is changed inside the initialized volume, then in the file
            "ALTER USER app PASSWORD",
            "MEILI_MASTER_KEY",
            // the port is on loopback now
            "APP_PUBLISH_ADDR",
            // the old image keeps the uploads in /data/up
            "tar -C /data -cf - up",
            "pg_dump -Fc",
            "git pull",
        ],
    );
    expect_order(
        &s,
        what,
        "cp config/.env.example config/.env",
        "pg_dump -Fc",
    );
    // the step that pulls comes after everything that needs the old checkout
    let pull = "Now `git pull`";
    expect_order(&s, what, "POSTGRES_PASSWORD=app", pull);
    expect_order(&s, what, "tar -C /data -cf - up", pull);
    expect_order(&s, what, "APP_PUBLISH_ADDR=", pull);
    expect_order(&s, what, pull, "ALTER USER app PASSWORD");
    // the general steps point at it before their first command
    let upgrade = ops_section("Upgrade");
    expect_order(
        &upgrade,
        "the section `Upgrade`",
        "Coming from a build before 0034",
        "git pull",
    );
    for file in [
        "config/.env.example",
        "docs/modules/ROOT/pages/install.adoc",
    ] {
        let text = read(file).replace("\n# ", " ").replace('\n', " ");
        assert!(
            text.contains("an existing database keeps its password"),
            "{file} does not say that an existing database keeps its password"
        );
    }
}

/// V1OPS-2: a restore can be rehearsed beside the live instance: a second project name, its
/// own host port and image name, and no step that touches the live volumes.
#[test]
fn a_restore_can_be_rehearsed_beside_the_live_instance() {
    let port = "\"${APP_PUBLISH_ADDR:-127.0.0.1}:${APP_PUBLISH_PORT:-3000}:3000\"";
    for file in ["src/docker/compose.yml", "src/docker/compose.podman.yml"] {
        assert!(read(file).contains(port), "{file}: the host port is fixed");
    }
    for file in COMPOSE_FILES {
        assert!(
            !read(file).contains("container_name"),
            "{file}: a fixed container name collides with a second project"
        );
    }
    let s = ops_section("Restore rehearsal beside the live instance");
    let what = "the section `Restore rehearsal beside the live instance`";
    expect_all(
        &s,
        what,
        &[
            "-p tracker-restore",
            "APP_PUBLISH_PORT=3001",
            "APP_PUBLISH_ADDR=127.0.0.1",
            "APP_IMAGE=",
            // no second worker beside the live one (notifications would be sent twice)
            "APP_ROLE=web",
            "http://127.0.0.1:3001/healthz",
            "dcr exec app cli secrets status",
            "dcr down -v",
            "Never remove the volumes of the live instance",
        ],
    );
    assert!(
        !s.contains("dc down"),
        "{what} must not stop the live instance"
    );
    // the helper is defined before it is used, and the clean-up comes last
    expect_order(&s, what, "dcr() {", "dcr build");
    expect_order(&s, what, "dcr exec app cli secrets status", "dcr down -v");
    // the plain restore no longer offers "remove the volumes" as the way to an empty instance
    let restore = ops_section("Restore");
    assert!(
        !restore.contains("removing the volumes"),
        "Restore still sends the reader to remove the live volumes"
    );
    assert!(restore.contains("Restore rehearsal beside the live instance"));
}

/// V1OPS-3: the store has four top-level directories; each place that lists them lists four.
#[test]
fn the_docs_name_all_four_blob_directories() {
    let moved = ops_section("Blob directory of earlier images");
    let reference = read("docs/modules/ROOT/pages/configuration.adoc");
    let setting = reference
        .lines()
        .find(|l| l.contains("APP__STORAGE__FS_LEGACY_ROOT"))
        .expect("fs_legacy_root in the configuration reference");
    let image = read("src/docker/Dockerfile");
    let comment = image
        .lines()
        .skip_while(|l| !l.starts_with("# One volume"))
        .take_while(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for (what, text) in [
        (
            "operations.adoc, Blob directory of earlier images",
            moved.as_str(),
        ),
        ("configuration.adoc, storage.fs_legacy_root", setting),
        ("the Dockerfile comment on the volume", comment.as_str()),
    ] {
        expect_all(text, what, &["`up`", "`img`", "`http`", "`orig`"]);
    }
}

/// V1OPS-4: the way back is complete: an empty database instead of `--clean` on a migrated
/// one, the blob directories back where the older image looks, and a manual reverse of 0034.
#[test]
fn the_way_back_is_complete() {
    let s = ops_section("Going back");
    let what = "the section `Going back`";
    expect_all(
        &s,
        what,
        &[
            "dc stop app",
            "dropdb -U app --force",
            "createdb -U app",
            "pg_restore --no-owner -U app -d app",
            "/data/blobs/$d",
            "mv ",
            "for d in up img http orig",
            "git checkout",
            "dc build",
        ],
    );
    assert!(!s.contains("--clean"), "{what} uses pg_restore --clean");
    expect_order(&s, what, "dc stop app", "dropdb -U app --force");
    expect_order(&s, what, "createdb -U app", "pg_restore --no-owner");
    expect_order(&s, what, "mv ", "git checkout");
    // the restore itself does not rely on --clean either (a started, migrated instance)
    let restore = ops_section("Restore");
    assert!(
        !restore.contains("--clean"),
        "Restore uses pg_restore --clean"
    );
    expect_all(
        &restore,
        "the section `Restore`",
        &["dropdb -U app --force", "createdb -U app"],
    );
    let notes = ops_section("Upgrade notes");
    expect_all(
        &notes,
        "the 0034 upgrade note",
        &[
            "ALTER TABLE secrets_keys DROP COLUMN backup_confirmed_at;",
            "DELETE FROM _sqlx_migrations WHERE version = 34;",
        ],
    );
}

/// V1OPS-5: one image for every container. The commented worker runs the image the `app`
/// service builds, the upgrade builds and replaces everything, and the page says that the
/// storage commands of a new image do not run before its server has started.
#[test]
fn every_container_runs_the_image_the_app_service_builds() {
    let base = read("src/docker/compose.yml");
    let image = "image: \"${APP_IMAGE:-localhost/tracker-app:local}\"";
    let app = &compose_services(&base)[0];
    assert_eq!(app.name, "app");
    assert!(app.lines.iter().any(|l| l.trim() == image), "app: {image}");
    assert!(app.lines.iter().any(|l| l.trim() == "build:"));
    let worker = commented_service(&base, "worker");
    assert!(worker.iter().any(|l| l.trim() == image), "worker: {image}");
    assert!(
        !worker.iter().any(|l| l.trim().starts_with("build:")),
        "the worker builds an image of its own"
    );
    assert!(worker.iter().any(|l| l.trim() == "pull_policy: never"));
    assert!(
        worker
            .iter()
            .any(|l| l.trim() == "command: [\"--role=worker\"]")
    );
    // the role of the app container is a variable (web beside workers, and in a rehearsal)
    assert!(
        app.lines
            .iter()
            .any(|l| l.trim() == "command: [\"--role=${APP_ROLE:-all}\"]")
    );
    let s = ops_section("Upgrade");
    let what = "the section `Upgrade`";
    assert!(
        !s.contains("dc build app"),
        "{what} builds the app service only"
    );
    expect_all(&s, what, &["`dc build`", "the same image", "cli storage"]);
}

/// V1OPS-6: what is under `up/` are import files; nothing promises more.
#[test]
fn the_backup_table_names_what_the_upload_directory_holds() {
    let s = ops_section("Backup");
    assert!(!s.contains("photos"), "Backup still lists photos under up/");
    assert!(
        !s.contains("avatars"),
        "Backup still lists avatars under up/"
    );
    expect_all(
        &s,
        "the section `Backup`",
        &["import files", "/data/blobs/orig"],
    );
}

/// V1OPS-7: the nginx snippet passes `Host` as it came (with the port) and says which
/// nginx the `http2` directive needs.
#[test]
fn the_nginx_snippet_keeps_the_host_header_and_names_its_version() {
    let proxy = read("docs/modules/ROOT/pages/reverse-proxy.adoc");
    assert!(!proxy.contains("Host $host;"), "`$host` drops the port");
    assert_eq!(
        proxy.matches("proxy_set_header Host $http_host;").count(),
        2
    );
    expect_all(
        &proxy,
        "reverse-proxy.adoc",
        &["http2 on;", "1.25.1", "listen 443 ssl http2;"],
    );
}

/// V1OPS-10: the restore waits for the services before it talks to them.
#[test]
fn the_restore_waits_for_the_services() {
    let s = ops_section("Restore");
    assert!(s.contains("dc up -d --wait postgres meilisearch"));
    assert!(!s.contains("dc up -d postgres meilisearch"));
}
