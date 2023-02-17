pub mod document;
pub mod dom;
pub mod font;
pub mod http;
pub mod layout;
pub mod paint;
pub mod parse;
pub mod viewport;

use std::{
    io::{Read, Write},
    net::TcpStream,
};

use backtrace::Backtrace;
use regex::{bytes::Captures as BinCaptures, bytes::Regex as BinRegex, Captures, Regex};
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

// to squelch rust-analyzer error on FONT_PATH in vscode, set
// WBE_FONT_PATH to /dev/null in rust-analyzer.cargo.extraEnv
pub const MARGIN: f32 = 16.0;
pub const FONT_SIZE: f32 = 16.0;
pub const FONT_NAME: &str = "Times New Roman";
pub const FONT_DATA: &[u8] = include_bytes!(env!("WBE_FONT_PATH"));

pub struct Split<'i>(Captures<'i>, &'i str);
pub struct BinSplit<'i>(BinCaptures<'i>, &'i [u8]);
impl<'i> Split<'i> {
    pub fn into_pair(self) -> (&'i str, &'i str) {
        (self.0.get(0).unwrap().as_str(), self.1)
    }
}
impl<'i> BinSplit<'i> {
    pub fn into_pair(self) -> (&'i [u8], &'i [u8]) {
        (self.0.get(0).unwrap().as_bytes(), self.1)
    }
}

pub fn dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|x| x.escape_ascii().to_string())
        .collect::<String>()
}

pub fn dump_backtrace(backtrace: Backtrace) {
    let prefix = &*format!("{}/src/", env!("CARGO_MANIFEST_DIR"));
    for (i, frame) in backtrace.frames().iter().enumerate() {
        if frame
            .symbols()
            .iter()
            .all(|s| s.filename().map_or(false, |x| !x.starts_with(prefix)))
        {
            break;
        }
        for (j, symbol) in frame.symbols().iter().enumerate() {
            if i == 0 && j == 0 {
                eprint!(">>> ");
            } else {
                eprint!("    ");
            }
            match symbol.name() {
                Some(name) => eprint!("{}", name),
                None => eprint!("?"),
            }
            match symbol.filename() {
                Some(filename) => {
                    eprint!(" ({}", filename.strip_prefix(prefix).unwrap().display())
                }
                None => eprint!(" (?"),
            }
            match symbol.lineno() {
                Some(lineno) => eprint!(":{})", lineno),
                None => eprint!(":?)"),
            }
            eprintln!();
        }
    }
}

pub fn parse<'i>(input: &'i str, pattern: &str) -> Option<Captures<'i>> {
    // +s (dot matches newlines), but no -u by default. -u affects ascii
    // character classes (\w\d\s), but it also makes dot (.) unusable.
    let pattern = format!("(?s){}", pattern);
    let re = Regex::new(&pattern).expect("failed to create Regex");

    re.captures(input)
}

pub fn parse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<BinCaptures<'i>> {
    // +s (dot matches newlines), -u (ascii \w\d\s and dot matches one octet).
    let pattern = format!("(?s-u){}", pattern);
    let re = BinRegex::new(&pattern).expect("failed to create Regex");

    re.captures(input)
}

pub fn lparse<'i>(input: &'i str, pattern: &str) -> Option<Captures<'i>> {
    parse(input, &format!("^{}", pattern))
}

pub fn lparse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<BinCaptures<'i>> {
    parse_bytes(input, &format!("^{}", pattern))
}

pub fn rparse<'i>(input: &'i str, pattern: &str) -> Option<Captures<'i>> {
    parse(input, &format!("{}$", pattern))
}

pub fn rparse_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<BinCaptures<'i>> {
    parse_bytes(input, &format!("{}$", pattern))
}

pub fn lparse_chomp<'i>(input: &mut &'i str, pattern: &str) -> Option<Captures<'i>> {
    let Some(result) = lparse(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.get(0).unwrap().as_str().len()..];

    Some(result)
}

pub fn lparse_chomp_bytes<'i>(input: &mut &'i [u8], pattern: &str) -> Option<BinCaptures<'i>> {
    let Some(result) = lparse_bytes(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[result.get(0).unwrap().as_bytes().len()..];

    Some(result)
}

pub fn rparse_chomp<'i>(input: &mut &'i str, pattern: &str) -> Option<Captures<'i>> {
    let Some(result) = rparse(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[..input.len() - result.get(0).unwrap().as_str().len()];

    Some(result)
}

pub fn rparse_chomp_bytes<'i>(input: &mut &'i [u8], pattern: &str) -> Option<BinCaptures<'i>> {
    let Some(result) = rparse_bytes(input, pattern) else { return None };

    // update input slice reference to unmatched part
    *input = &input[..input.len() - result.get(0).unwrap().as_bytes().len()];

    Some(result)
}

pub fn lparse_split<'i>(input: &'i str, pattern: &str) -> Option<Split<'i>> {
    let Some(result) = lparse(input, pattern) else { return None };
    let len = result.get(0).unwrap().as_str().len();

    Some(Split(result, &input[len..]))
}

pub fn lparse_split_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<BinSplit<'i>> {
    let Some(result) = lparse_bytes(input, pattern) else { return None };
    let len = result.get(0).unwrap().as_bytes().len();

    Some(BinSplit(result, &input[len..]))
}

pub fn rparse_split<'i>(input: &'i str, pattern: &str) -> Option<Split<'i>> {
    let Some(result) = rparse(input, pattern) else { return None };
    let len = result.get(0).unwrap().as_str().len();

    Some(Split(result, &input[..input.len() - len]))
}

pub fn rparse_split_bytes<'i>(input: &'i [u8], pattern: &str) -> Option<BinSplit<'i>> {
    let Some(result) = rparse_bytes(input, pattern) else { return None };
    let len = result.get(0).unwrap().as_bytes().len();

    Some(BinSplit(result, &input[..input.len() - len]))
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
