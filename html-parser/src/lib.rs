use eyre::bail;

use tracing::{debug, error, trace};
use wbe_dom::{Node, NodeData, Style};
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
    let mut html: Option<Node> = None;
    let mut css_files = vec![];

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

                ///////
                let css_file = wbe_css_parser::css_file(text);
                debug!(?css_file);
                if let Ok((_, css_file)) = css_file {
                    let mut html_style = html.clone().unwrap().data().style().clone();
                    for (selectors, declarations) in &css_file {
                        if !selectors.iter().any(|x| *x == (vec!["html"], vec![])) {
                            continue;
                        }
                        for &(name, value) in declarations {
                            match name {
                                "background-color" => {
                                    html_style.background_color = Some(dbg!(value).to_owned());
                                }
                                "color" => {
                                    html_style.color = Some(dbg!(value).to_owned());
                                }
                                _ => {}
                            }
                        }
                    }
                    html.clone().unwrap().data_mut().set_style(dbg!(html_style));
                    css_files.push(css_file);
                }
                ///////
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

                if &*element.name() == "html" {
                    html = Some(element.clone());
                } else {
                    for css_file in &css_files {
                        for (selectors, declarations) in css_file {
                            if !selectors
                                .iter()
                                .any(|x| *x == (vec![&element.name()], vec![]))
                            {
                                continue;
                            }
                            for &(name, value) in declarations {
                                match name {
                                    "background-color" => {
                                        let mut style = element.data().style();
                                        style.background_color = Some(dbg!(value).to_owned());
                                        element.data_mut().set_style(style);
                                    }
                                    "color" => {
                                        let mut style = element.data().style();
                                        style.color = Some(dbg!(value).to_owned());
                                        element.data_mut().set_style(style);
                                    }
                                    _ => {}
                                }
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
