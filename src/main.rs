fn main() {
    let code = file_forge::cli::run();
    if code != 0 {
        std::process::exit(code);
    }
}
