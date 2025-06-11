//! HTTP response type and helper methods.
//!
//! Cot uses the [`Response`](http::Response) type from the [`http`] crate
//! to represent outgoing HTTP responses. However, it also provides a
//! [`ResponseExt`] trait that contain various helper methods for working with
//! HTTP responses. These methods are used to create new responses with HTML
//! content types, redirects, and more. You probably want to have a `use`
//! statement for [`ResponseExt`] in your code most of the time to be able to
//! use these functions:
//!
//! ```
//! use cot::response::ResponseExt;
//! ```

use crate::error_page::ErrorPageTrigger;
use crate::html::Html;
use crate::{Body, StatusCode};

mod into_response;

pub use into_response::{
    IntoResponse, WithBody, WithContentType, WithExtension, WithHeader, WithStatus,
};

const RESPONSE_BUILD_FAILURE: &str = "Failed to build response";

/// HTTP response type.
pub type Response = http::Response<Body>;

mod private {
    pub trait Sealed {}
}

/// Extension trait for [`http::Response`] that provides helper methods for
/// working with HTTP response.
///
/// # Sealed
///
/// This trait is sealed since it doesn't make sense to be implemented for types
/// outside the context of Cot.
pub trait ResponseExt: Sized + private::Sealed {
    /// Create a new response builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::StatusCode;
    /// use cot::response::{Response, ResponseExt};
    ///
    /// let response = Response::builder()
    ///     .status(StatusCode::OK)
    ///     .body(cot::Body::empty())
    ///     .expect("Failed to build response");
    /// ```
    #[must_use]
    fn builder() -> http::response::Builder;

    /// Create a new redirect response.
    ///
    /// This creates a new [`Response`] object with a status code of
    /// [`StatusCode::SEE_OTHER`] and a location header set to the provided
    /// location.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::StatusCode;
    /// use cot::response::{Response, ResponseExt};
    ///
    /// let response = Response::new_redirect("http://example.com");
    /// ```
    ///
    /// # See also
    ///
    /// * [`crate::reverse_redirect!`] – a more ergonomic way to create
    ///   redirects to internal views
    #[must_use]
    fn new_redirect<T: Into<String>>(location: T) -> Self;
}

impl private::Sealed for Response {}

impl ResponseExt for Response {
    fn builder() -> http::response::Builder {
        http::Response::builder()
    }

    fn new_redirect<T: Into<String>>(location: T) -> Self {
        http::Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(http::header::LOCATION, location.into())
            .body(Body::empty())
            .expect(RESPONSE_BUILD_FAILURE)
    }
}

pub(crate) fn not_found_response(message: Option<String>) -> crate::Result<Response> {
    Html::new("404 Not Found")
        .with_status(StatusCode::NOT_FOUND)
        .with_extension(ErrorPageTrigger::NotFound { message })
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::BodyInner;
    use crate::headers::JSON_CONTENT_TYPE;
    use crate::response::{Response, ResponseExt};

    #[test]
    #[cfg(feature = "json")]
    fn response_new_json() {
        #[derive(serde::Serialize)]
        struct MyData {
            hello: String,
        }

        let data = MyData {
            hello: String::from("world"),
        };
        let response = crate::json::Json(data).into_response().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );
        match &response.body().inner {
            BodyInner::Fixed(fixed) => {
                assert_eq!(fixed, r#"{"hello":"world"}"#);
            }
            _ => {
                panic!("Expected fixed body");
            }
        }
    }

    #[test]
    fn response_new_redirect() {
        let location = "http://example.com";
        let response = Response::new_redirect(location);
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(http::header::LOCATION).unwrap(),
            location
        );
    }
}
