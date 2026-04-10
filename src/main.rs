fn main() {
    if let Err(error) = seshat::cli::run() {
        eprintln!("Erro: {error}");
        std::process::exit(1);
    }
}
