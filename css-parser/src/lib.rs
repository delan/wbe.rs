use std::fmt::Display;

use egui::Color32;
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag, take, take_until, take_while, take_while1},
    character::complete::{alpha1, anychar, one_of},
    combinator::{fail, map, map_parser, opt, peek, recognize},
    multi::{count, many0, many1, many_till, separated_list0, separated_list1},
    number::complete::float,
    sequence::{preceded, separated_pair, terminated, tuple},
    IResult, Parser,
};

pub fn own<'i>(
    parse: impl FnMut(&'i str) -> IResult<&str, &str> + Copy,
) -> impl FnMut(&'i str) -> IResult<&str, String> + Copy {
    move |input| map(parse, |x| x.to_owned())(input)
}

pub fn one<'i>(pred: impl Fn(char) -> bool) -> impl FnMut(&'i str) -> IResult<&str, &str> {
    move |input| {
        let (rest, result) = take(1usize)(input)?;
        if pred(result.chars().next().unwrap()) {
            Ok((rest, result))
        } else {
            fail(input)
        }
    }
}

pub fn is_css_space(c: char) -> bool {
    c.is_ascii_whitespace()
}

pub fn is_css_wordnum(c: char) -> bool {
    match c {
        c if !c.is_ascii() => true,
        c if c.is_ascii_alphanumeric() => true,
        '_' | '-' => true,
        _ => false,
    }
}

pub fn css_space(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_ascii_whitespace())(input)
}

#[rustfmt::skip]
pub fn css_ident(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        alt((
            tag("--"),
            recognize(tuple((
                opt(tag("-")),
                take(1usize).and_then(alpha1.or(tag("_"))),
            ))),
        )),
        take_while(|c| match c {
            c if c.is_ascii_alphanumeric() => true,
            '_' | '-' => true,
            _ => false,
        }),
    )))(input)
}

#[rustfmt::skip]
pub fn css_hash(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        tag("#"),
        take_while1(is_css_wordnum),
    )))(input)
}

#[rustfmt::skip]
pub fn css_selector(input: &str) -> IResult<&str, &str> {
    alt((
        alt((tag("*"), css_ident)),
        css_hash,
        recognize(tuple((tag("."), css_ident))),
    ))(input)
}

pub type CompoundSelector<'s> = Vec<String>;
pub fn css_selector_compound(input: &str) -> IResult<&str, CompoundSelector> {
    many1(own(css_selector))(input)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

#[rustfmt::skip]
pub fn css_selector_combinator(input: &str) -> IResult<&str, Combinator> {
    alt((
        map(css_space, |_| Combinator::Descendant),
        map(tag(">"), |_| Combinator::Child),
        map(tag("+"), |_| Combinator::NextSibling),
        map(tag("~"), |_| Combinator::SubsequentSibling),
    ))(input)
}

pub type ComplexSelector<'s> = (
    Vec<(CompoundSelector<'s>, Combinator)>,
    CompoundSelector<'s>,
);
pub fn css_selector_complex(input: &str) -> IResult<&str, ComplexSelector> {
    many_till(
        tuple((css_selector_compound, css_selector_combinator)),
        css_selector_compound,
    )(input)
}

pub type SelectorList<'s> = Vec<ComplexSelector<'s>>;
#[rustfmt::skip]
pub fn css_selector_list(input: &str) -> IResult<&str, SelectorList> {
    separated_list1(
        tuple((opt(css_space), tag(","), opt(css_space))),
        css_selector_complex,
    )(input)
}

pub type Declaration<'s> = (String, String);
pub type DeclarationList<'s> = Vec<Declaration<'s>>;
pub type Rule<'s> = (SelectorList<'s>, DeclarationList<'s>);
#[rustfmt::skip]
pub fn css_rule(input: &str) -> IResult<&str, (SelectorList, DeclarationList)> {
    let (rest, (selectors, _, _, _, declarations, _, _)) = tuple((
        css_selector_list,
        opt(css_space),
        tag("{"),
        opt(css_space),
        separated_list0(
            // Copy not implemented on returned closures
            // https://github.com/rust-lang/rust/issues/68307
            css_big_token(move |i| tag(";")(i)),
            separated_pair(
                own(css_big_token(css_ident)),
                css_big_token(move |i| tag(":")(i)),
                own(|i| recognize(many_till(anychar, tuple((opt(css_space), peek(one_of(";}"))))))(i)),
            ),
        ),
        many0(alt((tag(";"), css_space))),
        tag("}"),
    ))(input)?;

    Ok((rest, (selectors, declarations)))
}

pub fn css_comment(input: &str) -> IResult<&str, &str> {
    recognize(tuple((tag("/*"), many_till(anychar, tag("*/")))))(input)
}

#[rustfmt::skip]
// the Copy is because of https://github.com/rust-bakery/nom/issues/1044
pub fn css_big_token<'i, O: 'i>(parse: impl FnMut(&'i str) -> IResult<&str, O> + Copy) -> impl FnMut(&'i str) -> IResult<&str, O> + Copy {
    move |input| terminated(
        preceded(
            tuple((opt(css_space), opt(css_comment), opt(css_space))),
            parse,
        ),
        tuple((opt(css_space), opt(css_comment), opt(css_space))),
    )(input)
}

pub fn stag(x: &'static str) -> impl FnMut(&str) -> IResult<&str, &str> {
    move |input| css_big_token(move |i| tag(x)(i))(input)
}

fn rule_with_bad_selector(input: &str) -> IResult<&str, &str> {
    recognize(tuple((take_until("}"), tag("}"))))(input)
}

pub type RuleList<'s> = Vec<Rule<'s>>;
#[rustfmt::skip]
pub fn css_file(input: &str) -> IResult<&str, RuleList> {
    let mut input = input;
    let mut result = vec![];

    while !input.is_empty() {
        if let Ok((rest, rule)) = css_big_token(css_rule)(input) {
            result.push(rule);
            input = rest;
            continue;
        }
        if let Ok((rest, _)) = rule_with_bad_selector(input) {
            input = rest;
            continue;
        }
        // TODO warn
        input = &input[input.len()..];
    }

    Ok((input, result))
}

#[rustfmt::skip]
pub fn color_numeric(input: &str) -> IResult<&str, Color32> {
    let d8 = |input| map(tuple((float, opt(tag("%")))),
                         |(x,p)| (x / if p.is_some() { 255.0 / 100.0 } else { 1.0 }) as u8)(input);
    let h8 = |input| map(recognize(count(one(|x| x.is_ascii_hexdigit()), 2)),
                         |x| u8::from_str_radix(x,16).unwrap())(input);
    let h4 = |input| map(recognize(count(one(|x| x.is_ascii_hexdigit()), 1)),
                         |x| u8::from_str_radix(x,16).unwrap())(input);

    let (rest, (r, g, b, a)) = alt((
        map(tuple((tag("rgb"), opt(tag("a")), tag("("), d8, stag(","), d8, stag(","), d8, opt(preceded(stag(","), d8)), stag(")"))),
            |(_,_,_,r,_,g,_,b,a,_)| (r,g,b,a.unwrap_or(255))),
        map(tuple((tag("#"), h8, h8, h8, opt(h8))), |(_,r,g,b,a)| (r,g,b,a.unwrap_or(255))),
        map(tuple((tag("#"), h4, h4, h4, opt(h4))), |(_,r,g,b,a)| (17*r,17*g,17*b,17*a.unwrap_or(15))),
    ))(input)?;

    Ok((rest, Color32::from_rgba_unmultiplied(r, g, b, a)))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Zero,
    Percent(f32),
    Px(f32),
    Em(f32),
}

impl CssLength {
    pub fn parse(value: &str) -> Option<CssLength> {
        if let Ok(("", result)) = length(value) {
            Some(result)
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

#[rustfmt::skip]
pub fn length(input: &str) -> IResult<&str, CssLength> {
    alt((
        map(terminated(map_parser(is_a("-.0123456789"), float), tag("%")), CssLength::Percent),
        map(terminated(map_parser(is_a("-.0123456789"), float), tag("px")), CssLength::Px),
        map(terminated(map_parser(is_a("-.0123456789"), float), tag("em")), CssLength::Em),
        map(tag("0"), |_| CssLength::Zero),
    ))(input)
}

#[rustfmt::skip]
pub fn font_shorthand(input: &str) -> IResult<&str, (Vec<&str>, CssLength, Option<f32>, Vec<&str>)> {
    tuple((
        many0(css_big_token(css_ident)),
        css_big_token(length),
        opt(preceded(stag("/"), css_big_token(float))),
        separated_list1(stag(","), recognize(many1(css_big_token(css_ident)))),
    ))(input)
}

#[test]
#[rustfmt::skip]
fn test_css_file() {
    assert_eq!(color_numeric("#a0B1c2D3"), Ok(("", Color32::from_rgba_unmultiplied(0xA0, 0xB1, 0xC2, 0xD3))));
    assert_eq!(color_numeric("#A0b1C2"), Ok(("", Color32::from_rgba_unmultiplied(0xA0, 0xB1, 0xC2, 0xFF))));
    assert_eq!(color_numeric("#aBcD"), Ok(("", Color32::from_rgba_unmultiplied(0xAA, 0xBB, 0xCC, 0xDD))));
    assert_eq!(color_numeric("#AbC"), Ok(("", Color32::from_rgba_unmultiplied(0xAA, 0xBB, 0xCC, 0xFF))));

    assert_eq!(CssLength::parse("-1em"), Some(CssLength::Em(-1.0)));
    assert_eq!(CssLength::parse(".5em"), Some(CssLength::Em(0.5)));

    assert_eq!(css_ident("x{}"), Ok(("{}", "x")));
    assert_eq!(css_selector("x{}"), Ok(("{}", "x")));
    assert_eq!(css_selector_compound("x{}"), Ok(("{}", vec!["x".to_owned()])));
    assert_eq!(css_selector_compound("x.y#z{}"), Ok(("{}", vec!["x".to_owned(), ".y".to_owned(), "#z".to_owned()])));
    assert_eq!(css_selector_complex("x{}"), Ok(("{}", (vec![], vec!["x".to_owned()]))));
    assert_eq!(css_selector_complex("x.y#z a>b+c~d{}"), Ok(("{}", (
        vec![
            (vec!["x".to_owned(), ".y".to_owned(), "#z".to_owned()], Combinator::Descendant),
            (vec!["a".to_owned()], Combinator::Child),
            (vec!["b".to_owned()], Combinator::NextSibling),
            (vec!["c".to_owned()], Combinator::SubsequentSibling),
        ],
        vec!["d".to_owned()],
    ))));
    assert_eq!(css_selector_list("x{}"), Ok(("{}", vec![(vec![], vec!["x".to_owned()])])));
    assert_eq!(css_rule("x{}"), Ok(("", (vec![(vec![], vec!["x".to_owned()])], vec![]))));
    assert_eq!(css_file("x{}"), Ok(("", vec![(vec![(vec![], vec!["x".to_owned()])], vec![])])));
    assert_eq!(css_file("*{}x{}"), Ok(("", vec![(vec![(vec![], vec!["x".to_owned()])], vec![])])));
    assert_eq!(css_file(include_str!("../../browser/src/html.css")), Ok(("", vec![])));
}
