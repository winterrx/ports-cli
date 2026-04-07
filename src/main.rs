fn main() {
    ports_cli::run("ports", std::env::args().skip(1));
}
