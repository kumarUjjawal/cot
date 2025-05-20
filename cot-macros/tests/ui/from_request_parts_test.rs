use cot::axum::extract::RequestParts;
use cot::http::Request;
use cot_macros::FromRequestParts;

#[tokio::test]
async fn test_derive_from_request_parts_success() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct TestContext {
        user_id: i32,
        username: String,
    }

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(10_i32);
    parts.extensions.insert("test_user".to_string());

    let context = TestContext::from_request_parts(&mut parts).await.unwrap();

    assert_eq!(
        context,
        TestContext {
            user_id: 10,
            username: "test_user".to_string()
        }
    );
}

#[tokio::test]
async fn test_derive_from_request_parts_different_types() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct TestContext {
        value_i64: i64,
        value_bool: bool,
        value_string: String,
    }

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(12345_i64);
    parts.extensions.insert(true);
    parts.extensions.insert("another_test".to_string());

    let context = TestContext::from_request_parts(&mut parts).await.unwrap();

    assert_eq!(
        context,
        TestContext {
            value_i64: 12345,
            value_bool: true,
            value_string: "another_test".to_string()
        }
    );
}

#[tokio::test]
async fn test_derive_from_request_parts_missing_extension() {
    #[derive(FromRequestParts, Debug)]
    struct TestContext {
        missing_value: i32,
    }

    let mut parts = RequestParts::new(Request::default());

    let result = TestContext::from_request_parts(&mut parts).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_derive_from_request_parts_empty_struct() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct EmptyContext {}

    let mut parts = RequestParts::new(Request::default());
    let context = EmptyContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(EmptyContext {}, context);
}
