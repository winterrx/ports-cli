fn main() {
    ports_cli::run("whoisonport", std::env::args().skip(1));
}
