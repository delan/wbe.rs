use egui::Color32;
use eyre::eyre;
use tracing::{debug, error, info, trace, warn};

use wbe_css_parser::{css_file, css_hash, css_ident, RuleList};
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
    assert_eq!(node.r#type(), NodeType::Element);

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
            if let Ok(("", selector)) = css_ident(&selector) {
                // check if the simple type selector matches
                if !node.name().eq_ignore_ascii_case(&selector) {
                    continue;
                }
            } else if let Ok(("", selector)) = css_hash(&selector) {
                // check if the simple id selector matches
                let id = selector.strip_prefix("#").unwrap();
                if node.attr("id").map_or(true, |x| &*x != id) {
                    continue;
                }
            }
            trace!(selector, node = %*node.data());
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
                            style.margin = Some(result);
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
