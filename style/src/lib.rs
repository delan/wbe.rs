use tracing::{error, trace, warn};
use wbe_css_parser::{css_file, css_ident};
use wbe_dom::Node;

pub fn resolve_styles(css_text: &str, dom_tree: &Node) -> eyre::Result<()> {
    let rules = match css_file(css_text) {
        Ok(("", result)) => result,
        Ok((rest, result)) => {
            warn!("trailing text in css file: {:?}", rest);
            result
        }
        Err(error) => {
            error!(?error);
            return Ok(()); // TODO
        }
    };
    for (selectors, declarations) in rules {
        for (selector, combinators) in selectors {
            if !combinators.is_empty() {
                continue; // TODO
            }
            if selector.len() != 1 {
                continue; // TODO
            }
            let selector = selector[0];
            if css_ident(selector).is_err() {
                continue; // TODO
            }
            for node in dom_tree
                .descendants()
                .filter(|x| x.name().eq_ignore_ascii_case(selector))
            {
                trace!(selector, node = %*node.data());
                let mut style = node.data().style();
                for &(name, value) in &declarations {
                    match name {
                        "background-color" => style.background_color = Some(value.to_owned()),
                        "color" => style.color = Some(value.to_owned()),
                        _ => {}
                    }
                }
                node.data_mut().set_style(style);
            }
        }
    }

    Ok(())
}
