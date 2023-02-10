use std::{
    env::args,
    io::{stdout, Write},
    str,
};

use wbe::*;

fn main() -> eyre::Result<()> {
    // log to stdout (level configurable by RUST_LOG=debug)
    tracing_subscriber::fmt::init();

    let url = args()
        .nth(1)
        .unwrap_or("http://example.org/index.html".to_owned());
    load(&url)?;

    Ok(())
}

fn load(url: &str) -> eyre::Result<()> {
    let (_headers, body) = http::request(url)?;
    show(&body)?;

    Ok(())
}

fn show(mut body: &[u8]) -> eyre::Result<()> {
    while let Some(token) = lparse_chomp(&mut body, "<.+?>|[^<]+") {
        if !token.starts_with(b"<") {
            stdout().write_all(token)?;
        }
    }

    Ok(())
}
