use cot_macros::{AsFormField, SelectChoice};

#[derive(SelectChoice, AsFormField, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    Published,
    Archived,
}

#[derive(SelectChoice, AsFormField, Debug, PartialEq, Eq)]
enum MixedCase {
    FooBar,
    BAZ,
    SnakeCase,
}

#[derive(SelectChoice, AsFormField, Debug, PartialEq, Eq)]
enum WithOverrides {
    #[select_choice(id = "custom", name = "Custom Display")]
    Custom,
    #[select_choice(name = "Bar Human")]
    Bar,
    #[select_choice(id = "baz_id")]
    Baz,
    Default,
}

fn main() {}
