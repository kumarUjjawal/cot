use cot::form::fields::SelectChoice;

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    Published,
    Archived,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum EmptyEnum {}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum MixedCase {
    FooBar,
    BAZ,
    snake_case,
}

fn main() {}
