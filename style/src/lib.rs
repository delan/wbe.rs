use egui::Color32;
use eyre::eyre;
use tracing::{debug, error, info, instrument, trace, warn};

use wbe_css_parser::{
    css_file, css_hash, css_ident, Combinator, ComplexSelector, CompoundSelector, RuleList,
};
use wbe_dom::{
    style::{CssBorder, CssColor, CssFontStyle, CssFontWeight, CssLength, CssQuad, CssWidth},
    Node, NodeType, Style,
};

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

#[instrument(skip(dom_tree, rules))]
pub fn resolve_styles(dom_tree: &Node, rules: &RuleList) -> eyre::Result<()> {
    for node in dom_tree.descendants() {
        match node.r#type() {
            NodeType::Document => unreachable!(),
            NodeType::Comment => {
                // do nothing
            }
            NodeType::Text => {
                // inherit only inherited properties
                let style = node.parent().unwrap().data().style().new_inherited();
                node.data_mut().set_style(style);
            }
            NodeType::Element => {
                // inherit only inherited properties
                let mut style = node.parent().unwrap().data().style().new_inherited();
                let parent_style = node.parent().unwrap().data().style();

                // apply ‘font-size’ and ‘color’ first
                apply(&node, rules, &mut style, &parent_style, Some("font-size"))?;
                apply(&node, rules, &mut style, &parent_style, Some("color"))?;

                // then apply everything else
                apply(&node, rules, &mut style, &parent_style, None)?;

                // update style in element
                info!(node = %*node.data(), %style);
                debug!(?style);
                node.data_mut().set_style(style);
            }
        }
    }

    Ok(())
}

fn apply(
    node: &Node,
    rules: &RuleList,
    style: &mut Style,
    parent_style: &Style,
    property: Option<&str>,
) -> eyre::Result<()> {
    // apply matching rules in file order (TODO cascade)
    for (selectors, declarations) in rules {
        for complex in selectors {
            if !match_complex(node, complex) {
                continue;
            }
            for (name, value) in declarations {
                if property.map_or(false, |x| x != name) {
                    continue;
                }
                match &**name {
                    "display" => style.display = Some(value.to_owned()),
                    "margin" => {
                        if let Some(result) = CssQuad::parse_shorthand(value, CssLength::parse) {
                            style.margin = Some(result);
                        }
                    }
                    "padding" => {
                        if let Some(result) = CssQuad::parse_shorthand(value, CssLength::parse) {
                            style.padding = Some(result);
                        }
                    }
                    "border" => {
                        if let Some(result) = CssBorder::parse_shorthand(value) {
                            style.border = Some(CssQuad::one(result));
                        }
                    }
                    "font-size" => {
                        style.font_size = Some(
                            CssLength::parse(value).map_or(parent_style.font_size(), |x| {
                                x.resolve(parent_style.font_size(), parent_style.font_size())
                            }),
                        );
                    }
                    "font-weight" => {
                        style.font_weight = match &**value {
                            "normal" => Some(CssFontWeight::Normal),
                            "bold" => Some(CssFontWeight::Bold),
                            _ => style.font_weight,
                        }
                    }
                    "font-style" => {
                        style.font_style = match &**value {
                            "normal" => Some(CssFontStyle::Normal),
                            "italic" => Some(CssFontStyle::Italic),
                            _ => style.font_style,
                        }
                    }
                    "width" => {
                        style.width = match &**value {
                            "auto" => Some(CssWidth::Auto),
                            other => CssLength::parse(other).map(CssWidth::Length),
                        }
                    }
                    "background-color" => {
                        if let Some(result) = CssColor::parse(value) {
                            // if ‘currentColor’, use self ‘color’
                            style.background_color = Some(result);
                        }
                    }
                    "color" => {
                        if let Some(result) = CssColor::parse(value) {
                            // if ‘currentColor’, use parent ‘color’
                            style.color = Some(result.resolve(parent_style.color()));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn match_compound(node: &Node, compound: &Vec<String>) -> bool {
    for simple in compound {
        if simple == "*" {
            continue;
        } else if let Ok(("", selector)) = css_ident(&simple) {
            // check if the simple type selector matches
            if !node.name().eq_ignore_ascii_case(&selector) {
                return false;
            }
        } else if let Some(Ok(("", selector))) = simple.strip_prefix(".").map(css_ident) {
            // check if the simple class selector matches
            if node
                .attr("class")
                .map_or(true, |x| x.split_ascii_whitespace().all(|x| x != selector))
            {
                return false;
            }
        } else if let Ok(("", selector)) = css_hash(&simple) {
            // check if the simple id selector matches
            let id = selector.strip_prefix("#").unwrap();
            if node.attr("id").map_or(true, |x| &*x != id) {
                return false;
            }
        }
    }

    true
}

fn match_complex(node: &Node, (combinators, compound): &ComplexSelector) -> bool {
    trace!(node = %*node.data(), ?combinators, ?compound);
    if !match_compound(node, compound) {
        return false;
    }
    let mut next = node.clone();
    for (compound, combinator) in combinators {
        if let Some(result) = match combinator {
            Combinator::Descendant => next.walk_up().find(|x| match_compound(x, compound)),
            Combinator::Child => next.walk_up().take(1).find(|x| match_compound(x, compound)),
            Combinator::SubsequentSibling => next.walk_left().find(|x| match_compound(x, compound)),
            Combinator::NextSibling => next
                .walk_left()
                .take(1)
                .find(|x| match_compound(x, compound)),
        } {
            next = result;
        } else {
            return false;
        }
    }

    true
}

#[test]
#[rustfmt::skip]
fn test() -> eyre::Result<()> {
    use wbe_html_parser::parse_html;

    let dom = parse_html("<html><body><p><b></b><i></i><a id=b class='c d'>x</a>")?;
    let a = dom.children()[0].children()[0].children()[0].children()[2].clone();
    assert!(match_compound(&a, &compound([])));
    assert!(match_compound(&a, &compound(["*"])));
    assert!(match_compound(&a, &compound(["a"])));
    assert!(match_compound(&a, &compound(["#b"])));
    assert!(match_compound(&a, &compound([".c"])));
    assert!(match_compound(&a, &compound([".d"])));
    assert!(match_compound(&a, &compound(["*", "#b", ".c", ".d"])));
    assert!(match_compound(&a, &compound(["a", ".d", ".c", "#b"])));
    assert!(match_complex(&a, &complex(["p", "a"], [Combinator::Descendant])));
    assert!(match_complex(&a, &complex(["body", "a"], [Combinator::Descendant])));
    assert!(match_complex(&a, &complex(["html", "a"], [Combinator::Descendant])));
    assert!(match_complex(&a, &complex(["p", "a"], [Combinator::Child])));
    assert!(match_complex(&a, &complex(["i", "a"], [Combinator::NextSibling])));
    assert!(match_complex(&a, &complex(["i", "a"], [Combinator::SubsequentSibling])));
    assert!(match_complex(&a, &complex(["b", "a"], [Combinator::SubsequentSibling])));

    fn compound(simples: impl IntoIterator<Item = &'static str>) -> CompoundSelector<'static> {
        simples.into_iter().map(|x| x.to_owned()).collect()
    }

    fn complex(simples: impl IntoIterator<Item = &'static str>, combinators: impl IntoIterator<Item = Combinator>) -> ComplexSelector<'static> {
        let mut result = simples.into_iter().map(|x| vec![x.to_owned()]).collect::<Vec<_>>();
        let base = result.pop().unwrap();

        (result.into_iter().zip(combinators.into_iter()).collect(), base)
    }

    Ok(())
}
