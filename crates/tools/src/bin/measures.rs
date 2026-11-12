//! measures <journal directory>: the measures report, from the journal alone.
//! The journal is checked first (format::chain); a journal that does not
//! check gets no report.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir] = args.as_slice() else {
        eprintln!("usage: measures <journal directory>");
        std::process::exit(2);
    };
    let path = std::path::Path::new(dir).join(format::record::JOURNAL);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("measures: {}: {e}", path.display());
            std::process::exit(1);
        }
    };
    match format::chain::check_journal(&text) {
        Ok(entries) => println!("{}", tools::measures::measure(&entries)),
        Err(e) => {
            eprintln!("measures: {e}");
            std::process::exit(1);
        }
    }
}
