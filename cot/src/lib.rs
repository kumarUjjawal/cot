//! Cot is an easy to use, modern, and fast web framework for Rust. It has
//! been designed to be familiar if you've ever used
//! [Django](https://www.djangoproject.com/), and easy to learn if you haven't.
//! It's a batteries-included framework built on top of
//! [axum](https://github.com/tokio-rs/axum).
//!
//! ## Features
//!
//! * **Easy to use API** — in many ways modeled after Django, Cot's API is
//!   designed to be easy to use and intuitive. Sensible defaults make it for
//!   easy rapid development, while the API is still empowering you when needed.
//!   The documentation is a first-class citizen in Cot, making it easy to find
//!   what you're looking for.
//! * **ORM integration** — Cot comes with its own ORM, allowing you to interact
//!   with your database in a way that feels Rusty and intuitive. Rust types are
//!   the source of truth, and the ORM takes care of translating them to and
//!   from the database, as well as creating the migrations automatically.
//! * **Type safe** — wherever possible, Cot uses Rust's type system to prevent
//!   common mistakes and bugs. Not only views are taking advantage of the
//!   Rust's type system, but also the ORM, the admin panel, and even the
//!   templates. All that to catch errors as early as possible.
//! * **Admin panel** — Cot comes with an admin panel out of the box, allowing
//!   you to manage your app's data with ease. Adding new models to the admin
//!   panel is stupidly simple, making it a great tool not only for rapid
//!   development and debugging, but with its customization options, also for
//!   production use.
//! * **Secure by default** — security should be opt-out, not opt-in. Cot takes
//!   care of making your web apps secure by default, defending it against
//!   common modern web vulnerabilities. You can focus on building your app, not
//!   securing it.
//!
//! ## Guide
//!
//! This is an API reference for Cot, which might not be the best place to
//! start learning Cot. For a more gentle introduction, see the
//! [Cot guide](https://cot.rs/guide/latest/).
//!
//! ## Examples
//!
//! To see examples of how to use Cot, see the
//! [examples in the repository](https://github.com/cot-rs/cot/tree/master/examples).

#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![cfg_attr(
    docsrs,
    feature(doc_cfg, doc_auto_cfg, rustdoc_missing_doc_code_examples)
)]
#![cfg_attr(docsrs, warn(rustdoc::missing_doc_code_examples))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/cot-rs/media/6585c518/logo/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/cot-rs/media/6585c518/logo/favicon.svg"
)]

extern crate self as cot;

#[cfg(feature = "db")]
pub mod db;
mod error;
pub mod form;
mod headers;
// Not public API. Referenced by macro-generated code.
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
pub mod admin;
pub mod auth;
mod body;
pub mod cli;
pub mod common_types;
pub mod config;
mod error_page;
#[macro_use]
pub(crate) mod handler;
pub mod html;
#[cfg(feature = "json")]
pub mod json;
pub mod middleware;
#[cfg(feature = "openapi")]
pub mod openapi;
pub mod project;
pub mod request;
pub mod response;
pub mod router;
mod serializers;
pub mod session;
pub mod static_files;
#[cfg(feature = "test")]
pub mod test;
pub(crate) mod utils;

#[cfg(feature = "openapi")]
pub use aide;
pub use body::Body;
/// An attribute macro that defines an end-to-end test function for a
/// Cot-powered app.
///
/// This is primarily useful for use with the
/// [`TestServerBuilder`](cot::test::TestServerBuilder) struct, which allows you
/// to run a full-fledged Cot server in a test environment.
///
/// Internally, this is equivalent to `#[tokio::test]` with the test body
/// wrapped in a [`tokio::task::LocalSet`] to allow for running non-`Send` async
/// code in the test.
///
/// # Examples
///
/// ```
/// use cot::test::TestServerBuilder;
///
/// struct TestProject;
/// impl cot::Project for TestProject {}
///
/// #[cot::e2e_test]
/// async fn test_server() -> cot::Result<()> {
///     let server = TestServerBuilder::new(TestProject).start().await;
///
///     server.close().await;
///     Ok(())
/// }
/// ```
pub use cot_macros::e2e_test;
/// An attribute macro that defines an entry point to a Cot-powered app.
///
/// This macro is meant to wrap a function returning a structure implementing
/// [`cot::Project`]. It should just initialize a [`cot::Project`] and return
/// it, while the macro takes care of initializing an async runtime, creating a
/// CLI and running the app.
///
/// # Examples
///
/// ```no_run
/// use cot::project::RegisterAppsContext;
/// use cot::{App, AppBuilder, Project};
///
/// struct HelloApp;
///
/// impl App for HelloApp {
///     fn name(&self) -> &'static str {
///         env!("CARGO_PKG_NAME")
///     }
/// }
///
/// struct HelloProject;
/// impl Project for HelloProject {
///     fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
///         apps.register_with_views(HelloApp, "");
///     }
/// }
///
/// #[cot::main]
/// fn main() -> impl Project {
///     HelloProject
/// }
/// ```
pub use cot_macros::main;
pub use cot_macros::test;
pub use error::Error;
#[cfg(feature = "openapi")]
pub use schemars;
pub use {bytes, http};

pub use crate::handler::{BoxedHandler, RequestHandler};
pub use crate::project::{
    run, run_at, run_cli, App, AppBuilder, Bootstrapper, Project, ProjectContext,
};

/// A type alias for a result that can return a [`cot::Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;

/// Derives `FromRequestParts` for a struct.
///
/// This derive macro is intended to help extract multiple request parts
/// in an Axum handler using a custom struct. Each field's type must
/// implement `FromRequestParts`.
///
/// # Example
///
/// ```rust
/// use axum::extract::FromRequestParts;
/// use http::request::Parts;
///
/// #[derive(cot_macros::FromRequestParts)]
/// struct AuthenticatedUser {
///     user_id: UserId,
///     session: SessionInfo,
/// }
/// ```
///
/// This allows using `AuthenticatedUser` as a single extractor:
///
/// ```rust
/// async fn handler(user: AuthenticatedUser) {
///     // You now have both user_id and session
/// }
/// ```
pub use cot_macros::FromRequestParts;
pub use cot_macros::SelectChoice;
