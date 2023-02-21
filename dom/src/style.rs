use std::fmt::{Debug, Display};

use egui::Color32;

use tracing::warn;
use wbe_core::FONT_SIZE;
use wbe_css_parser::color_numeric;

lazy_static::lazy_static! {
    pub static ref INITIAL_STYLE: Style = Style {
        display: Some("inline".to_owned()),
        margin: Some(CssQuad::one(CssLength::Zero)),
        padding: Some(CssQuad::one(CssLength::Zero)),
        border: Some(CssQuad::one(CssBorder::none())),
        font_size: Some(FONT_SIZE),
        font_weight: Some(CssFontWeight::Normal),
        font_style: Some(CssFontStyle::Normal),
        width: Some(CssWidth::Auto),
        background_color: Some(CssColor::Other(Color32::TRANSPARENT)),
        color: Some(Color32::BLACK),
    };
}

#[derive(Debug, Clone)]
pub struct Style {
    pub display: Option<String>,
    pub margin: Option<CssQuad<CssLength>>,
    pub padding: Option<CssQuad<CssLength>>,
    pub border: Option<CssQuad<CssBorder>>,
    pub font_size: Option<f32>,
    pub font_weight: Option<CssFontWeight>,
    pub font_style: Option<CssFontStyle>,
    pub width: Option<CssWidth>,
    pub background_color: Option<CssColor>,
    pub color: Option<Color32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssColor {
    CurrentColor,
    Other(Color32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Zero,
    Percent(f32),
    Px(f32),
    Em(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssQuad<T: Debug + Clone + Copy + PartialEq> {
    pub top: Option<T>,
    pub right: Option<T>,
    pub bottom: Option<T>,
    pub left: Option<T>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssDisplay {
    None,
    Inline,
    Block,
    InlineBlock,
    ListItem,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontWeight {
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssWidth {
    Auto,
    Length(CssLength),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssBorder {
    pub width: Option<CssLength>,
    pub color: Option<CssColor>,
}

impl Style {
    pub fn empty() -> Self {
        Self {
            display: None,
            margin: None,
            padding: None,
            border: None,
            font_size: None,
            font_weight: None,
            font_style: None,
            width: None,
            background_color: None,
            color: None,
        }
    }

    pub fn initial() -> &'static Self {
        &*INITIAL_STYLE
    }

    pub fn new_inherited(&self) -> Self {
        Self {
            font_size: self.font_size,
            font_weight: self.font_weight,
            font_style: self.font_style,
            color: self.color.clone(),
            ..Self::initial().clone()
        }
    }

    pub fn apply(&mut self, other: &Style) {
        self.display = other.display.clone().or(self.display.clone());
        self.background_color = other
            .background_color
            .clone()
            .or(self.background_color.clone());
        self.color = other.color.clone().or(self.color.clone());
    }

    pub fn display(&self) -> CssDisplay {
        match &**self
            .display
            .as_ref()
            .unwrap_or_else(|| Self::initial().display.as_ref().unwrap())
        {
            "none" => CssDisplay::None,
            "inline" => CssDisplay::Inline,
            "block" => CssDisplay::Block,
            _ => CssDisplay::Inline,
        }
    }

    pub fn margin(&self) -> CssQuad<CssLength> {
        self.get(|s| s.margin)
    }

    pub fn margin_side(
        &self,
        getter: impl Fn(&CssQuad<CssLength>) -> Option<CssLength>,
    ) -> CssLength {
        getter(&self.get(|s| s.margin))
            .unwrap_or_else(|| getter(&Self::initial().margin.unwrap()).unwrap())
    }

    pub fn padding(&self) -> CssQuad<CssLength> {
        self.get(|s| s.padding)
    }

    pub fn padding_side(
        &self,
        getter: impl Fn(&CssQuad<CssLength>) -> Option<CssLength>,
    ) -> CssLength {
        getter(&self.get(|s| s.padding))
            .unwrap_or_else(|| getter(&Self::initial().padding.unwrap()).unwrap())
    }

    pub fn border_width(&self) -> CssQuad<CssLength> {
        self.get(|s| s.border).flat_map(|x| x.width)
    }

    pub fn border_top_width(&self) -> CssLength {
        self.get(|s| s.border_side(CssQuad::top).width)
    }

    pub fn border_top_color(&self) -> CssColor {
        self.get(|s| s.border_side(CssQuad::top).color)
    }

    pub fn border_right_width(&self) -> CssLength {
        self.get(|s| s.border_side(CssQuad::right).width)
    }

    pub fn border_right_color(&self) -> CssColor {
        self.get(|s| s.border_side(CssQuad::right).color)
    }

    pub fn border_bottom_width(&self) -> CssLength {
        self.get(|s| s.border_side(CssQuad::bottom).width)
    }

    pub fn border_bottom_color(&self) -> CssColor {
        self.get(|s| s.border_side(CssQuad::bottom).color)
    }

    pub fn border_left_width(&self) -> CssLength {
        self.get(|s| s.border_side(CssQuad::left).width)
    }

    pub fn border_left_color(&self) -> CssColor {
        self.get(|s| s.border_side(CssQuad::left).color)
    }

    pub fn font_size(&self) -> f32 {
        self.get(|s| s.font_size)
    }

    pub fn font_weight(&self) -> CssFontWeight {
        self.get(|s| s.font_weight)
    }

    pub fn font_style(&self) -> CssFontStyle {
        self.get(|s| s.font_style)
    }

    pub fn box_width(&self, percent_base: f32) -> f32 {
        let font_size = self.font_size();
        match self.get(|s| s.width) {
            CssWidth::Auto => percent_base,
            CssWidth::Length(x) => {
                let margin_left = self
                    .margin_side(CssQuad::left)
                    .resolve(percent_base, font_size);
                let padding_left = self
                    .padding_side(CssQuad::left)
                    .resolve(percent_base, font_size);
                let border_left = self
                    .border_left_width()
                    .resolve_no_percent(font_size)
                    .unwrap_or(0.0);
                let border_right = self
                    .border_right_width()
                    .resolve_no_percent(font_size)
                    .unwrap_or(0.0);
                let padding_right = self
                    .padding_side(CssQuad::right)
                    .resolve(percent_base, font_size);
                let margin_right = self
                    .margin_side(CssQuad::right)
                    .resolve(percent_base, font_size);

                x.resolve(percent_base, font_size)
                    + margin_left
                    + padding_left
                    + border_left
                    + border_right
                    + padding_right
                    + margin_right
            }
        }
    }

    pub fn background_color(&self) -> CssColor {
        self.get(|s| s.background_color)
    }

    pub fn color(&self) -> Color32 {
        self.get(|s| s.color)
    }

    fn get<T>(&self, getter: impl Fn(&Self) -> Option<T>) -> T {
        getter(self).unwrap_or_else(|| getter(Self::initial()).unwrap())
    }

    fn border_side(&self, getter: impl Fn(&CssQuad<CssBorder>) -> Option<CssBorder>) -> CssBorder {
        getter(&self.get(|s| s.border))
            .unwrap_or_else(|| getter(&Self::initial().border.unwrap()).unwrap())
    }
}

impl CssColor {
    pub fn parse(value: &str) -> Option<CssColor> {
        fn rgba(rgba32: u32) -> Color32 {
            Color32::from_rgba_unmultiplied(
                (rgba32 >> 24) as _,
                (rgba32 >> 16 & 255) as _,
                (rgba32 >> 8 & 255) as _,
                (rgba32 & 255) as _,
            )
        }

        if value.eq_ignore_ascii_case("currentcolor") {
            return Some(CssColor::CurrentColor);
        }

        Some(CssColor::Other(match value {
            // impl defined in CSS1, defined in CSS2
            x if x.eq_ignore_ascii_case("maroon") => rgba(0x800000ff),
            x if x.eq_ignore_ascii_case("red") => rgba(0xff0000ff),
            x if x.eq_ignore_ascii_case("yellow") => rgba(0xffff00ff),
            x if x.eq_ignore_ascii_case("olive") => rgba(0x808000ff),
            x if x.eq_ignore_ascii_case("purple") => rgba(0x800080ff),
            x if x.eq_ignore_ascii_case("fuchsia") => rgba(0xff00ffff),
            x if x.eq_ignore_ascii_case("white") => rgba(0xffffffff),
            x if x.eq_ignore_ascii_case("lime") => rgba(0x00ff00ff),
            x if x.eq_ignore_ascii_case("green") => rgba(0x008000ff),
            x if x.eq_ignore_ascii_case("navy") => rgba(0x000080ff),
            x if x.eq_ignore_ascii_case("blue") => rgba(0x0000ffff),
            x if x.eq_ignore_ascii_case("aqua") => rgba(0x00ffffff),
            x if x.eq_ignore_ascii_case("teal") => rgba(0x008080ff),
            x if x.eq_ignore_ascii_case("black") => rgba(0x000000ff),
            x if x.eq_ignore_ascii_case("silver") => rgba(0xc0c0c0ff),
            x if x.eq_ignore_ascii_case("gray") => rgba(0x808080ff),

            // not defined in CSS1, defined in CSS2
            x if x.eq_ignore_ascii_case("orange") => rgba(0xffA50000),

            // defined after CSS2
            x if x.eq_ignore_ascii_case("transparent") => rgba(0x00000000),
            x if x.eq_ignore_ascii_case("rebeccapurple") => rgba(0x663399FF),

            other => {
                if let Ok(("", result)) = color_numeric(other) {
                    result
                } else {
                    warn!("unknown color {:?}", other);
                    return None;
                }
            }
        }))
    }

    pub fn resolve(&self, current_color: Color32) -> Color32 {
        match self {
            CssColor::CurrentColor => current_color,
            CssColor::Other(other) => *other,
        }
    }
}

impl CssLength {
    pub fn parse(value: &str) -> Option<CssLength> {
        if let Some(number) = value.strip_suffix("%") {
            number.parse::<f32>().ok().map(CssLength::Percent)
        } else if let Some(number) = value.strip_suffix("px") {
            number.parse::<f32>().ok().map(CssLength::Px)
        } else if let Some(number) = value.strip_suffix("em") {
            number.parse::<f32>().ok().map(CssLength::Em)
        } else {
            None
        }
    }

    pub fn resolve(&self, percent_base: f32, em_base: f32) -> f32 {
        match self {
            CssLength::Zero => 0.0,
            CssLength::Percent(x) => x / 100.0 * percent_base,
            CssLength::Px(x) => *x,
            CssLength::Em(x) => x * em_base,
        }
    }

    pub fn resolve_no_percent(&self, em_base: f32) -> Option<f32> {
        match self {
            CssLength::Percent(_) => None,
            other => Some(other.resolve(f32::NAN, em_base)),
        }
    }
}

impl<T: Debug + Clone + Copy + PartialEq> CssQuad<T> {
    pub fn one(all: impl Into<Option<T>> + Copy) -> Self {
        Self::four(all, all, all, all)
    }

    pub fn two(
        vertical: impl Into<Option<T>> + Copy,
        horizontal: impl Into<Option<T>> + Copy,
    ) -> Self {
        Self::four(vertical, horizontal, vertical, horizontal)
    }

    pub fn three(
        top: impl Into<Option<T>> + Copy,
        horizontal: impl Into<Option<T>> + Copy,
        bottom: impl Into<Option<T>> + Copy,
    ) -> Self {
        Self::four(top, horizontal, bottom, horizontal)
    }

    pub fn four(
        top: impl Into<Option<T>> + Copy,
        right: impl Into<Option<T>> + Copy,
        bottom: impl Into<Option<T>> + Copy,
        left: impl Into<Option<T>> + Copy,
    ) -> Self {
        Self {
            top: top.into(),
            right: right.into(),
            bottom: bottom.into(),
            left: left.into(),
        }
    }

    pub fn top(&self) -> Option<T> {
        self.top
    }

    pub fn right(&self) -> Option<T> {
        self.right
    }

    pub fn bottom(&self) -> Option<T> {
        self.bottom
    }

    pub fn left(&self) -> Option<T> {
        self.left
    }

    pub fn flat_map<U: Debug + Clone + Copy + PartialEq>(
        &self,
        f: impl Fn(T) -> Option<U>,
    ) -> CssQuad<U> {
        CssQuad::four(
            self.top.map(&f).flatten(),
            self.right.map(&f).flatten(),
            self.bottom.map(&f).flatten(),
            self.left.map(&f).flatten(),
        )
    }

    pub fn parse_shorthand(value: &str, parser: impl Fn(&str) -> Option<T>) -> Option<Self> {
        let value = value.split_ascii_whitespace().collect::<Vec<_>>();
        let result = match value[..] {
            [a] => Self::one(parser(a)),
            [a, b] => Self::two(parser(a), parser(b)),
            [a, b, c] => Self::three(parser(a), parser(b), parser(c)),
            [a, b, c, d] => Self::four(parser(a), parser(b), parser(c), parser(d)),
            _ => return None,
        };

        result.require_all_valid()
    }

    fn require_all_valid(self) -> Option<Self> {
        if self.top.is_none()
            || self.right.is_none()
            || self.bottom.is_none()
            || self.left.is_none()
        {
            return None;
        }

        Some(self)
    }
}

impl<T: Debug + Copy + Clone + PartialEq> Default for CssQuad<T> {
    fn default() -> Self {
        Self {
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }
}

impl CssWidth {
    pub fn resolve(&self, percent_base: f32, em_base: f32) -> f32 {
        match self {
            CssWidth::Auto => percent_base,
            CssWidth::Length(x) => x.resolve(percent_base, em_base),
        }
    }
}

impl CssBorder {
    fn none() -> CssBorder {
        Self {
            width: Some(CssLength::Zero),
            color: Some(CssColor::Other(Color32::from_rgb(255, 0, 255))),
        }
    }

    pub fn parse_shorthand(value: &str) -> Option<Self> {
        let mut result = Self::none();

        for value in value.split_ascii_whitespace() {
            match value {
                "0" => result.width = Some(CssLength::Zero),
                "solid" => {}
                other => {
                    if let Some(other) = CssLength::parse(other) {
                        result.width = Some(other);
                    } else if let Some(other) = CssColor::parse(other) {
                        result.color = Some(other);
                    } else {
                        return None;
                    }
                }
            }
        }

        Some(result)
    }
}

#[test]
pub fn parse() {
    assert_eq!(CssLength::parse("1em"), Some(CssLength::Em(1.0)));
    assert_eq!(
        CssBorder::parse_shorthand("1em solid black"),
        Some(CssBorder {
            width: Some(CssLength::Em(1.0)),
            color: Some(CssColor::Other(Color32::BLACK))
        })
    );
}

impl Display for CssWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Length(x) => write!(f, "{}", x),
        }
    }
}

impl Display for CssLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CssLength::Zero => write!(f, "0"),
            CssLength::Percent(x) => write!(f, "{}%", x),
            CssLength::Px(x) => write!(f, "{}px", x),
            CssLength::Em(x) => write!(f, "{}em", x),
        }
    }
}

impl Display for CssColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentColor => write!(f, "currentColor"),
            Self::Other(x) => write!(f, "#{:02x}{:02x}{:02x}{:02x}", x.r(), x.g(), x.b(), x.a()),
        }
    }
}

impl Display for CssBorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(x) = self.width {
            write!(f, "{}", x)
        } else {
            write!(f, "unset")
        }?;
        write!(f, " ")?;
        if let Some(x) = self.color {
            write!(f, "{}", x)
        } else {
            write!(f, "unset")
        }?;

        Ok(())
    }
}

impl<T: Debug + Clone + Copy + PartialEq + Display> Display for CssQuad<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("quad");
        if let Some(x) = self.top {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.right {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.bottom {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.left {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }

        t.finish()
    }
}

impl Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_struct("style");
        if let Some(x) = self.border {
            t.field("border", &format_args!("{}", x));
        }

        t.finish()
    }
}
