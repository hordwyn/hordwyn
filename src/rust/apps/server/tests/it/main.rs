//! The server's integration tests, one test binary (DEC-565): every file here is a module, so
//! the server is linked once instead of once per file. Run one module with
//! `cargo test -p server --test it -- item_page::`.
//!
//! Tests of all modules run as parallel threads of one process: nothing here may change
//! process-wide state (environment variables, the working directory, a global subscriber) or
//! use a fixed port, file or database name.

mod support;

mod admin;
mod admin_fns;
mod admin_pages;
mod auth_pages;
mod body_limit;
mod books;
mod brand;
mod cli;
mod home_page;
mod imports;
mod instance;
mod item_page;
mod latency;
mod library_page;
mod load;
mod manual;
mod member_pin;
mod notifications;
mod offline;
mod oidc;
mod ops;
mod phase1_fns;
mod read_timeout;
mod retention;
mod search_language;
mod security_headers;
mod setup_page;
mod sharing;
mod shell;
mod ssr_routes;
mod usage;
mod webhook;
