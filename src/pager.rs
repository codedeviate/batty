use crate::cli::PagingWhen;
use std::env;

/// Initialize the pager according to the requested mode. After this returns,
/// stdout writes are routed through the pager (if applicable). On `Never` or
/// when stdout isn't a TTY in `Auto` mode, this is a no-op.
pub fn setup(mode: PagingWhen) {
    match mode {
        PagingWhen::Never => return,
        PagingWhen::Auto => {
            if !atty_stdout() { return; }
        }
        PagingWhen::Always => {}
    }

    if env::var_os("LESS").is_none() {
        env::set_var("LESS", "-RF");
    }
    let pager = env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    pager::Pager::with_pager(&pager).setup();
}

fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
