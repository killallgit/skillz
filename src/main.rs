fn main() {
    if let Err(err) = skillz::cli::main() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
