use std::{
    collections::BTreeMap,
    io::BufRead,
    io::{BufReader, Read, Write},
    net::TcpStream,
    str,
    str::FromStr,
};

use eyre::bail;
use rustls_connector::RustlsConnector;
use tracing::{debug, instrument, trace};

use wbe_core::{dump, lparse, lparse_chomp, rparse_split, trim_ascii, ReadWriteStream};

#[instrument]
pub fn request(
    url: &str,
    base: Option<&str>,
) -> eyre::Result<(usize, BTreeMap<String, String>, Vec<u8>)> {
    let url = if let Some(data) = lparse(url, "data:([^;,]+)((?:;base64)?),(.*)") {
        assert_eq!(data.get(2).unwrap().as_str(), "");
        let mut result = vec![];
        let mut input = data.get(3).unwrap().as_str();
        while !input.is_empty() {
            if let Some(percent) = lparse_chomp(&mut input, "%[0-9A-Fa-f]{2}") {
                let percent = percent.get(0).unwrap().as_str();
                result.push(u8::from_str_radix(&percent[1..], 16).unwrap());
            } else {
                let (index, _) = input.char_indices().nth(1).unwrap();
                let (next, rest) = input.split_at(index);
                for octet in next.bytes() {
                    result.push(octet);
                }
                input = rest;
            }
        }
        return Ok((200, Default::default(), result));
    } else {
        let base = base.map(|x| Url::new(x, None).unwrap());
        Url::new(url, base.as_ref())?
    };

    let mut stream: Box<dyn ReadWriteStream> = match url.scheme() {
        "http:" => Box::new(TcpStream::connect((url.hostname(), url.port()))?),
        "https:" => {
            let connector = RustlsConnector::new_with_native_certs()?;
            let stream = TcpStream::connect((url.hostname(), url.port()))?;
            Box::new(connector.connect(url.hostname(), stream)?)
        }
        other => bail!("unknown scheme: {:?}", other),
    };
    write!(stream, "GET {} HTTP/1.0\r\n", url.path())?;
    write!(stream, "Host: {}:{}\r\n\r\n", url.hostname(), url.port())?;

    let mut stream = BufReader::new(stream);
    let mut received = vec![];
    assert_ne!(stream.read_until(b'\n', &mut received)?, 0);

    let line = received.strip_suffix(b"\r\n").unwrap();
    let [_version, status, _explanation] = line.splitn(3, |x| *x == b' ').collect::<Vec<_>>()[..]
        else { panic!("failed to parse response status line") };
    let Ok(Ok(status)) = str::from_utf8(status).map(usize::from_str)
        else { panic!("failed to parse response status code") };
    received.clear();

    let mut headers = BTreeMap::default();
    while stream.read_until(b'\n', &mut received)? > 0 {
        // TODO: hard-coding utf-8 is not correct in practice
        let line = str::from_utf8(&received)?;
        let line = line.strip_suffix("\r\n").unwrap();
        if line.is_empty() {
            break;
        }
        let (field, value) = line
            .split_once(":")
            .expect("failed to parse response header");
        debug!(field = field, value = value);
        headers.insert(
            trim_ascii(field).to_ascii_lowercase(),
            trim_ascii(value).to_owned(),
        );
        received.clear();
    }

    assert!(!headers.contains_key("transfer-encoding"));
    assert!(!headers.contains_key("content-encoding"));

    let mut body = vec![];
    stream.read_to_end(&mut body)?;
    debug!(body = dump(&body));

    Ok((status, headers, body))
}

#[derive(Debug)]
pub struct Url {
    scheme: String,
    hostname: String,
    port: u16,
    path: String,
}

impl Url {
    pub fn new(mut url: &str, base: Option<&Url>) -> eyre::Result<Self> {
        let Some(scheme) = lparse_chomp(&mut url, "[A-Za-z0-9-]+:")
            .map(|x| x.get(0).unwrap().as_str().to_owned())
            .or_else(|| base.map(|x| x.scheme.clone()))
            else { bail!("no scheme found but no base given") };
        let (hostname, port) = if lparse_chomp(&mut url, "//").is_some() {
            let Some(host) = lparse_chomp(&mut url, "[^/]+")
                .map(|x| x.get(0).unwrap().as_str())
                else { bail!("failed to chomp host") };
            let (port, hostname) = rparse_split(host, r":([0-9]+)")
                .map(|x| x.into_pair())
                .unwrap_or((
                    match scheme.as_ref() {
                        "http:" => ":80",
                        "https:" => ":443",
                        _ => unreachable!(),
                    },
                    host,
                ));
            let port = u16::from_str(&port[1..])?;

            (hostname.to_owned(), port)
        } else if let Some(base) = base {
            (base.hostname.clone(), base.port)
        } else {
            bail!("no host found but no base given")
        };
        let path = match url {
            "" => "/".to_owned(),
            other => {
                if other.starts_with("/") {
                    other.to_owned()
                } else if let Some(base) = base {
                    let (_, base) = rparse_split(&base.path, "[^/]*")
                        .map(|x| x.into_pair())
                        .unwrap();
                    base.to_owned() + other
                } else {
                    bail!("relative path found but no base given")
                }
            }
        };
        trace!(url, scheme, hostname, port, path);

        Ok(Self {
            scheme,
            hostname,
            port,
            path,
        })
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}
