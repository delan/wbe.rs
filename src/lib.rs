pub mod document;
pub mod font;
pub mod http;
pub mod paint;
pub mod viewport;

use std::{
    io::{Read, Write},
    net::TcpStream,
};

use regex::Regex;
use rustls_connector::TlsStream;

#[macro_export]
macro_rules! dbg_bytes {
    ($val:expr) => {
        match $val {
            x => {
                eprintln!(
                    "[{}:{}] {} = b\"{}\"",
                    file!(),
                    line!(),
                    stringify!($val),
                    $crate::dump(&x[..])
                );
                x
            }
        }
    };
}

pub fn dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|x| x.escape_ascii().to_string())
        .collect::<String>()
}

pub fn parse<'i>(input: &'i str, pattern: &str) -> Option<&'i str> {
    // +s (dot matches newlines), but no -u by default. -u affects ascii
    // character classes (\w\d\s), but it also makes dot (.) unusable.
    let pattern = format!("(?s){}", pattern);
    let re = Regex::new(&pattern).expect("failed to create Regex");
    let Some(result) = re.find(input) else { return None };

    Some(result.as_str())
}

pub fn parse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<&'i [u8]> {
    // +s (dot matches newlines), -u (ascii \w\d\s and dot matches one octet).
    let pattern = format!("(?s-u){}", pattern);
    let re = regex::bytes::Regex::new(&pattern).expect("failed to create Regex");
    let Some(result) = re.find(input) else { return None };

    Some(result.as_bytes())
}

pub fn lparse<'i>(input: &'i str, pattern: &str) -> Option<&'i str> {
    parse(input, &format!("^{}", pattern))
}

pub fn lparse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<&'i [u8]> {
    parse_bytes(input, &format!("^{}", pattern))
}

pub fn rparse<'i>(input: &'i str, pattern: &str) -> Option<&'i str> {
    parse(input, &format!("{}$", pattern))
}

pub fn rparse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<&'i [u8]> {
    parse_bytes(input, &format!("{}$", pattern))
}

pub fn lparse_chomp<'i>(input: &mut &'i str, pattern: &'static str) -> Option<&'i str> {
    let Some(result) = lparse(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.len()..];

    Some(result)
}

pub fn lparse_chomp_bytes<'i>(input: &mut &'i [u8], pattern: &'static str) -> Option<&'i [u8]> {
    let Some(result) = lparse_bytes(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.len()..];

    Some(result)
}

pub fn rparse_chomp<'i>(input: &mut &'i str, pattern: &'static str) -> Option<&'i str> {
    let Some(result) = rparse(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.len()..];

    Some(result)
}

pub fn rparse_chomp_bytes<'i>(input: &mut &'i [u8], pattern: &'static str) -> Option<&'i [u8]> {
    let Some(result) = rparse_bytes(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.len()..];

    Some(result)
}

pub fn lparse_split<'i>(input: &'i str, pattern: &'static str) -> Option<(&'i str, &'i str)> {
    let Some(result) = lparse(input, pattern) else { return None };

    Some((result, &input[result.len()..]))
}

pub fn lparse_split_bytes<'i>(
    input: &'i [u8],
    pattern: &'static str,
) -> Option<(&'i [u8], &'i [u8])> {
    let Some(result) = lparse_bytes(input, pattern) else { return None };

    Some((result, &input[result.len()..]))
}

pub fn rparse_split<'i>(input: &'i str, pattern: &'static str) -> Option<(&'i str, &'i str)> {
    let Some(result) = rparse(input, pattern) else { return None };

    Some((result, &input[result.len()..]))
}

pub fn rparse_split_bytes<'i>(
    input: &'i [u8],
    pattern: &'static str,
) -> Option<(&'i [u8], &'i [u8])> {
    let Some(result) = rparse_bytes(input, pattern) else { return None };

    Some((result, &input[result.len()..]))
}

pub fn trim_ascii(mut input: &str) -> &str {
    lparse_chomp(&mut input, r"\s+");
    rparse_chomp(&mut input, r"\s+");

    input
}

pub fn trim_ascii_bytes(mut input: &[u8]) -> &[u8] {
    lparse_chomp_bytes(&mut input, r"\s+");
    rparse_chomp_bytes(&mut input, r"\s+");

    input
}

pub trait ReadWriteStream: Read + Write {}
impl ReadWriteStream for TcpStream {}
impl<S: Read + Write> ReadWriteStream for TlsStream<S> {}