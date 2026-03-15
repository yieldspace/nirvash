fn main() {
    print!(
        "{}",
        docker_compose_model::format_summary(&docker_compose_model::plan_summary())
    );
}
