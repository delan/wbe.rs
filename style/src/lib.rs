use eyre::eyre;
use tracing::{trace, warn};
use wbe_css_parser::{css_file, css_ident, RuleList};
use wbe_dom::{Node, NodeType};

pub fn parse_css_file(text: &str) -> eyre::Result<RuleList> {
    match css_file(text) {
        Ok(("", result)) => Ok(result),
        Ok((rest, result)) => {
            warn!("trailing text in css file: {:?}", rest);
            Ok(result)
        }
        Err(error) => Err(eyre!("failed to parse css file: {:?}", error)),
    }
}

pub fn resolve_styles(dom_tree: &Node, rules: &RuleList) -> eyre::Result<()> {
    for node in dom_tree.descendants() {
        match node.r#type() {
            NodeType::Document => unreachable!(),
            NodeType::Comment => {
                // do nothing
            }
            NodeType::Text => {
                // inherit everything from element or #document
                let style = node.parent().unwrap().data().style();
                node.data_mut().set_style(style);
            }
            NodeType::Element => {
                // inherit only inherited properties
                let mut style = node.parent().unwrap().data().style().new_inherited();

                // apply matching rules in file order (TODO cascade)
                for (selectors, declarations) in rules {
                    for (base, combinators) in selectors {
                        if !combinators.is_empty() {
                            continue; // TODO
                        }
                        if base.len() != 1 {
                            continue; // TODO
                        }
                        let selector = &base[0];
                        if css_ident(&selector).is_err() {
                            continue; // TODO
                        }
                        // check if the simple type selector matches
                        if !node.name().eq_ignore_ascii_case(&selector) {
                            continue;
                        }
                        trace!(selector, node = %*node.data());
                        for (name, value) in declarations {
                            match &**name {
                                "background-color" => {
                                    style.background_color = Some(value.to_owned())
                                }
                                "color" => style.color = Some(value.to_owned()),
                                _ => {}
                            }
                        }
                    }
                }

                // update style in element
                node.data_mut().set_style(style);
            }
        }
    }

    Ok(())
}
