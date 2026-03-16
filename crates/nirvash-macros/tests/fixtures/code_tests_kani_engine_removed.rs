use nirvash_macros::code_tests;

#[code_tests(
    profiles = [
        removed = {
            coverage = [transitions],
            engines = [kani(depth = 4)],
        },
    ],
)]
struct Spec;

fn main() {}
