use eyre::bail;

use tracing::{error, trace};
use wbe_dom::{Node, NodeData};
use wbe_html_lexer::{html_token, HtmlToken};

// ([if the child is one of these], [the stack must not end with this sequence])
const NO_NEST: &[(&[&str], &[&str])] = &[
    (
        &["p", "table", "form", "h1", "h2", "h3", "h4", "h5", "h6"],
        &["p"],
    ),
    (&["li"], &["li"]),
    (&["dt", "dd"], &["dt"]),
    (&["dt", "dd"], &["dd"]),
    (&["tr"], &["tr"]),
    (&["tr"], &["tr", "td"]),
    (&["tr"], &["tr", "th"]),
    (&["td", "th"], &["td"]),
    (&["td", "th"], &["th"]),
];
const SELF_CLOSING: &[&str] = &[
    "!doctype", "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
    "param", "source", "track", "wbr",
];

pub fn parse_html(response_body: &str) -> eyre::Result<Node> {
    let mut parent = Node::new(NodeData::Document);
    let mut stack = vec![parent.clone()];
    let mut names_stack: Vec<&str> = vec![];
    let mut input = &*response_body;

    while !input.is_empty() {
        let (rest, token) = match html_token(input) {
            Ok(result) => result,
            // Err(nom::Err::Incomplete(_)) => ("", HtmlToken::Text(input)),
            Err(e) => bail!("{}; input={:?}", e, input),
        };
        match token {
            HtmlToken::Comment(text) => {
                parent.append(&[Node::comment(text.to_owned())]);
            }
            HtmlToken::Script(attrs, text) => {
                let attrs = attrs.into_iter().map(|(n, v)| (n.to_owned(), v)).collect();
                parent.append(&[Node::element("script".to_owned(), attrs)
                    .append(&[Node::text(text.to_owned())])]);
            }
            HtmlToken::Style(attrs, text) => {
                let attrs = attrs.into_iter().map(|(n, v)| (n.to_owned(), v)).collect();
                parent
                    .append(&[Node::element("style".to_owned(), attrs)
                        .append(&[Node::text(text.to_owned())])]);
            }
            HtmlToken::Tag(false, name, attrs) => {
                let attrs = attrs.into_iter().map(|(n, v)| (n.to_owned(), v)).collect();
                let element = Node::element(name.to_owned(), attrs);

                for &(child_names, suffix) in NO_NEST {
                    if child_names.contains(&&*name) {
                        if names_stack.ends_with(suffix) {
                            trace!(true, name, ?child_names, ?suffix, ?names_stack);
                            for _ in 0..suffix.len() {
                                let _ = stack.pop().unwrap();
                                let _ = names_stack.pop().unwrap();
                                parent = parent.parent().unwrap();
                            }
                        }
                    }
                }

                parent.append(&[element.clone()]);

                if !SELF_CLOSING.contains(&&*name) {
                    stack.push(element.clone());
                    names_stack.push(name);
                    parent = element;
                }
            }
            HtmlToken::Tag(true, name, _attrs) => {
                if let Some((i, _)) = names_stack
                    .iter()
                    .enumerate()
                    .rfind(|(_, x)| x.eq_ignore_ascii_case(name))
                {
                    for _ in 0..(names_stack.len() - i) {
                        let _ = stack.pop().unwrap();
                        let _ = names_stack.pop().unwrap();
                        parent = parent.parent().unwrap();
                    }
                } else {
                    error!("failed to find match for closing tag: {:?}", name);
                }
            }
            HtmlToken::Text(text) => {
                parent.append(&[Node::text(text.to_owned())]);
            }
        }
        input = rest;
    }

    Ok(stack[0].clone())
}
