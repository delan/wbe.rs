use egui::Color32;
use eyre::eyre;
use tracing::{debug, error, trace, warn};

use wbe_css_parser::{css_file, css_hash, css_ident, RuleList};
use wbe_dom::{
    style::{resolve_length, CssFontStyle, CssFontWeight, CssLength, CssWidth},
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
                let parent_font_size = style.font_size.unwrap();

                // apply ‘font-size’ first
                apply(
                    &node,
                    rules,
                    &mut style,
                    parent_font_size,
                    Some("font-size"),
                )?;

                // then apply everything else
                apply(&node, rules, &mut style, parent_font_size, None)?;

                // update style in element
                debug!(node = %*node.data(), style = ?style);
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
    parent_font_size: f32,
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
                    "font-size" => {
                        style.font_size = Some(parse_length(value).map_or(parent_font_size, |x| {
                            resolve_length(x, parent_font_size, parent_font_size)
                        }));
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
                            other => parse_length(other).map(CssWidth::Length),
                        }
                    }
                    "background-color" => {
                        style.background_color = Some(parse_color(value));
                    }
                    "color" => {
                        style.color = Some(parse_color(value));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn parse_color(color: &str) -> Color32 {
    match color {
        "transparent" => Color32::TRANSPARENT,
        "blue" => Color32::BLUE,
        "white" => Color32::WHITE,
        "black" => Color32::BLACK,
        "rgb(204,0,0)" => Color32::from_rgb(204, 0, 0),
        "#FC0" => Color32::from_rgb(0xFF, 0xCC, 0x00),
        "#663399" => Color32::from_rgb(0x66, 0x33, 0x99),
        "#008080" => Color32::from_rgb(0x00, 0x80, 0x80),
        other => {
            error!("unknown color {:?}", other);
            Color32::TEMPORARY_COLOR
        }
    }
}

pub fn parse_length(text: &str) -> Option<CssLength> {
    if let Some(number) = text.strip_suffix("%") {
        number.parse::<f32>().ok().map(CssLength::Percent)
    } else if let Some(number) = text.strip_suffix("px") {
        number.parse::<f32>().ok().map(CssLength::Px)
    } else if let Some(number) = text.strip_suffix("em") {
        number.parse::<f32>().ok().map(CssLength::Em)
    } else {
        None
    }
}
