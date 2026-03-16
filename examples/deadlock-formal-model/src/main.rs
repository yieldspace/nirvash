fn main() {
    print!(
        "{}",
        deadlock_formal_model::format_summary(&deadlock_formal_model::deadlock_summary())
    );
}
