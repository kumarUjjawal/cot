use cot_macros::SelectChoice;

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    Published,
    Archived,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum MixedCase {
    FooBar,
    BAZ,
    SnakeCase,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum WithOverrides {
    #[select_choice(id = "custom", name = "Custom Display")]
    Custom,
    #[select_choice(name = "Bar Human")]
    Bar,
    #[select_choice(id = "baz_id")]
    Baz,
    // No attribute: fallback to default
    Default,
}

fn main() {}

// The following should fail to compile: macro must not support empty enums
/*
#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum EmptyEnum {}
*/
