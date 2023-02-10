use std::{
    collections::BTreeMap,
    env::args,
    io::BufRead,
    io::{stdout, BufReader, Read, Write},
    net::TcpStream,
    str,
    str::FromStr,
};

use rustls_connector::RustlsConnector;
use tracing::{debug, instrument};

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
    let (_headers, body) = request(url)?;
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

#[instrument]
fn request(url: &str) -> eyre::Result<(BTreeMap<Vec<u8>, Vec<u8>>, Vec<u8>)> {
    let mut url = url.as_bytes();
    let scheme = lparse_chomp(&mut url, "https?:").expect("failed to chomp url scheme");
    lparse_chomp(&mut url, "//").expect("failed to chomp url //");
    let (host, path) = lparse_split(url, "[^/]+").expect("failed to split url host/path");
    let (hostname, port) = rparse_split(host, r":\d+").unwrap_or((
        host,
        match scheme {
            b"http:" => b":80",
            b"https:" => b":443",
            _ => unreachable!(),
        },
    ));
    let host = str::from_utf8(host)?;
    let hostname = str::from_utf8(hostname)?;
    let port = u16::from_str(str::from_utf8(&port[1..])?)?;
    let path = match path {
        b"" => "/",
        x => str::from_utf8(x)?,
    };

    let mut stream: Box<dyn ReadWriteStream> = match scheme {
        b"http:" => Box::new(TcpStream::connect((hostname, port))?),
        b"https:" => {
            let connector = RustlsConnector::new_with_native_certs()?;
            let stream = TcpStream::connect((hostname, port))?;
            Box::new(connector.connect(hostname, stream)?)
        }
        _ => unreachable!(),
    };
    write!(stream, "GET {} HTTP/1.0\r\n", path)?;
    write!(stream, "Host: {}\r\n\r\n", host)?;

    let mut stream = BufReader::new(stream);
    let mut received = vec![];
    assert_ne!(stream.read_until(b'\n', &mut received)?, 0);

    let line = received.strip_suffix(b"\r\n").unwrap();
    let [version, status, explanation] = line.splitn(3, |x| *x == b' ').collect::<Vec<_>>()[..]
        else { panic!("failed to parse response status line") };
    assert_eq!(
        status,
        b"200",
        "unexpected {:?} {:?} {:?}",
        dump(version),
        dump(status),
        dump(explanation)
    );
    received.clear();

    let mut headers = BTreeMap::default();
    while stream.read_until(b'\n', &mut received)? > 0 {
        let line = received.strip_suffix(b"\r\n").unwrap();
        if line.is_empty() {
            break;
        }
        let [field, value] = line.splitn(2, |x| *x == b':').collect::<Vec<_>>()[..]
            else { panic!("failed to parse response header") };
        debug!(field = dump(field), value = dump(value));
        headers.insert(
            trim_ascii(field).to_ascii_lowercase(),
            trim_ascii(value).to_owned(),
        );
        received.clear();
    }

    assert!(!headers.contains_key(&b"transfer-encoding"[..]));
    assert!(!headers.contains_key(&b"content-encoding"[..]));

    let mut body = vec![];
    stream.read_to_end(&mut body)?;
    debug!(body = dump(&body));

    Ok((headers, body))
}
