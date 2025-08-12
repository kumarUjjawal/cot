use cot::openapi::ApiOperationResponse;
use cot_macros::ApiOperationResponse as DeriveApiOperationResponse;
use cot::response::IntoResponse;

#[derive(DeriveApiOperationResponse)]
enum MyResponse {
    A(Dummy),
    B(Dummy),
}

struct Dummy;

impl IntoResponse for Dummy {
    fn into_response(self) -> cot::Result<cot::response::Response> {
        unimplemented!()
    }
}

impl ApiOperationResponse for Dummy {}

fn main() {}
