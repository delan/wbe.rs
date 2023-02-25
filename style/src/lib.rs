#![feature(stmt_expr_attributes)]

use eyre::eyre;
use paste::paste;
use tracing::{debug, instrument, trace, warn};

use wbe_css_parser::{
    css_declaration_list, css_file, css_hash, css_ident, Combinator, ComplexSelector, CssLength,
    DeclarationList, RuleList,
};
use wbe_dom::{
    style::{
        CssBorder, CssColor, CssFont, CssFontStyle, CssFontWeight, CssHeight, CssQuad,
        CssTextAlign, CssWidth, INITIAL_STYLE,
    },
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

pub fn parse_style_attr(text: &str) -> eyre::Result<DeclarationList> {
    match css_declaration_list(text) {
        Ok(("", result)) => Ok(result),
        Ok((rest, result)) => {
            warn!("trailing text in @style attr: {:?}", rest);
            Ok(result)
        }
        Err(error) => Err(eyre!("failed to parse @style attr: {:?}", error)),
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
                let inline = node
                    .attr("style")
                    .map(|x| parse_style_attr(&x).ok())
                    .flatten();

                // apply ‘font-size’ and ‘color’ first
                apply(&node, rules, &mut style, &parent_style, Some("font-size"))?;
                apply(&node, rules, &mut style, &parent_style, Some("color"))?;
                if let Some(ref inline) = inline {
                    apply_declarations(
                        &node,
                        &inline,
                        &mut style,
                        &parent_style,
                        Some("font-size"),
                    )?;
                    apply_declarations(&node, &inline, &mut style, &parent_style, Some("color"))?;
                }

                // then apply everything else
                apply(&node, rules, &mut style, &parent_style, None)?;
                if let Some(ref inline) = inline {
                    apply_declarations(&node, &inline, &mut style, &parent_style, None)?;
                }

                // update style in element
                trace!(?style);
                node.data_mut().set_style(style);
            }
        }
    }

    Ok(())
}

macro_rules! trbl {
    ($style:ident, $node:ident, $name:ident, $value:ident, $field:ident, $side:ident, $parse:expr) => {{
        if let Some(result) = $parse {
            paste!(*$style.[<$field _mut>]().[<$side _mut>](INITIAL_STYLE.$field())) = result;
            debug!($node = %*$node.data(), $name, $value);
            continue;
        }
    }};
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
            apply_declarations(node, declarations, style, parent_style, property)?;
        }
    }

    Ok(())
}

fn apply_declarations(
    node: &Node,
    declarations: &[(String, String)],
    style: &mut Style,
    parent_style: &Style,
    property: Option<&str>,
) -> eyre::Result<()> {
    for (name, value) in declarations {
        if property.map_or(false, |x| x != name) {
            continue;
        }
        match &**name {
            "display" => {
                style.display = Some(value.to_owned());
                debug!(node = %*node.data(), name, value);
                continue;
            }
            "margin" => {
                if let Some(result) = CssQuad::parse_shorthand(value, CssLength::parse) {
                    style.margin = result;
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "margin-top" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, margin, top, CssLength::parse(value));
            }
            "margin-right" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, margin, right, CssLength::parse(value));
            }
            "margin-bottom" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, margin, bottom, CssLength::parse(value));
            }
            "margin-left" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, margin, left, CssLength::parse(value));
            }
            "padding" => {
                if let Some(result) = CssQuad::parse_shorthand(value, CssLength::parse) {
                    style.padding = result;
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "padding-top" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, padding, top, CssLength::parse(value));
            }
            "padding-right" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, padding, right, CssLength::parse(value));
            }
            "padding-bottom" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, padding, bottom, CssLength::parse(value));
            }
            "padding-left" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, padding, left, CssLength::parse(value));
            }
            "border" => {
                if let Some(result) = CssBorder::parse_shorthand(value) {
                    style.border = CssQuad::one(result);
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "border-width" => {
                #[rustfmt::skip]
                if let Some(result) = CssQuad::parse_shorthand(value, CssLength::parse) {
                    style.border_mut().top_mut(INITIAL_STYLE.border()).width = Some(*result.top_unwrap());
                    style.border_mut().right_mut(INITIAL_STYLE.border()).width = Some(*result.right_unwrap());
                    style.border_mut().bottom_mut(INITIAL_STYLE.border()).width = Some(*result.bottom_unwrap());
                    style.border_mut().left_mut(INITIAL_STYLE.border()).width = Some(*result.left_unwrap());
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "border-color" => {
                #[rustfmt::skip]
                if let Some(result) = CssQuad::parse_shorthand(value, CssColor::parse) {
                    style.border_mut().top_mut(INITIAL_STYLE.border()).color = Some(*result.top_unwrap());
                    style.border_mut().right_mut(INITIAL_STYLE.border()).color = Some(*result.right_unwrap());
                    style.border_mut().bottom_mut(INITIAL_STYLE.border()).color = Some(*result.bottom_unwrap());
                    style.border_mut().left_mut(INITIAL_STYLE.border()).color = Some(*result.left_unwrap());
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "border-top" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, border, top, CssBorder::parse_shorthand(value));
            }
            "border-right" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, border, right, CssBorder::parse_shorthand(value));
            }
            "border-bottom" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, border, bottom, CssBorder::parse_shorthand(value));
            }
            "border-left" => {
                #[rustfmt::skip]
                trbl!(style, node, name, value, border, left, CssBorder::parse_shorthand(value));
            }
            "text-align" => {
                if let Some(result) = if value.eq_ignore_ascii_case("left") {
                    Some(CssTextAlign::Left)
                } else if value.eq_ignore_ascii_case("right") {
                    Some(CssTextAlign::Right)
                } else if value.eq_ignore_ascii_case("center") {
                    Some(CssTextAlign::Center)
                } else {
                    None
                } {
                    style.text_align = Some(result);
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "font" => {
                if value == "inherit" {
                    style.font = parent_style.font.clone();
                    continue;
                }
                if let Some((mut property, size)) = CssFont::parse_shorthand(value) {
                    property.size =
                        Some(size.resolve(parent_style.font_size(), parent_style.font_size()));
                    style.font = Some(property);
                    continue;
                }
            }
            "font-size" => {
                let mut property = style.font.take().unwrap_or_else(|| CssFont::none());
                property.size = Some(
                    CssLength::parse(value).map_or(parent_style.font_size(), |x| {
                        x.resolve(parent_style.font_size(), parent_style.font_size())
                    }),
                );
                style.font = Some(property);
                continue;
            }
            "font-weight" => {
                let mut property = style.font.take().unwrap_or_else(|| CssFont::none());
                property.weight = Some(match &**value {
                    "normal" => CssFontWeight::Normal,
                    "bold" => CssFontWeight::Bold,
                    _ => style.font_weight(),
                });
                style.font = Some(property);
                continue;
            }
            "font-style" => {
                let mut property = style.font.take().unwrap_or_else(|| CssFont::none());
                property.style = Some(match &**value {
                    "normal" => CssFontStyle::Normal,
                    "italic" => CssFontStyle::Italic,
                    _ => style.font_style(),
                });
                style.font = Some(property);
                continue;
            }
            "width" => {
                if let Some(result) = CssWidth::parse(value) {
                    style.width = Some(result);
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "height" => {
                if let Some(result) = CssHeight::parse(value) {
                    style.height = Some(result);
                    debug!(node = %*node.data(), name, value);
                    continue;
                }
            }
            "background" | "background-color" => {
                // TODO implement rest of shorthand
                let value = match value.as_ref() {
                    "none" => "transparent",
                    other => other,
                };
                if let Some(result) = CssColor::parse(value) {
                    // if ‘currentColor’, use self ‘color’
                    style.background_color = Some(result);
                    continue;
                }
            }
            "color" => {
                if let Some(result) = CssColor::parse(value) {
                    // if ‘currentColor’, use parent ‘color’
                    style.color = Some(result.resolve(parent_style.color()));
                    continue;
                }
            }
            other => {
                warn!(node = %*node.data(), "unknown property {:?} (value {:?})", other, value);
                continue;
            }
        }
        warn!(node = %*node.data(), "invalid value for property {}: {:?}", name, value);
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
    use wbe_css_parser::CompoundSelector;
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
