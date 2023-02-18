use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_till, take_until, take_while, take_while1},
    character::complete::{alpha1, anychar, char, one_of},
    combinator::{fail, map_parser, opt, peek, recognize},
    multi::{many0, many1, many_till, separated_list0, separated_list1},
    sequence::{preceded, separated_pair, terminated, tuple},
    IResult, Parser,
};

pub fn one(input: &str, pred: impl Fn(char) -> bool) -> IResult<&str, &str> {
    let (rest, result) = take(1usize)(input)?;
    if pred(result.chars().next().unwrap()) {
        Ok((rest, result))
    } else {
        fail(input)
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
        css_ident,
        css_hash,
        recognize(tuple((tag("."), css_ident))),
    ))(input)
}

pub type CompoundSelector<'s> = Vec<&'s str>;
pub fn css_selector_compound(input: &str) -> IResult<&str, CompoundSelector> {
    many1(css_selector)(input)
}

pub fn css_selector_combinator(input: &str) -> IResult<&str, &str> {
    alt((css_space, tag(">"), tag("+"), tag("~")))(input)
}

pub type ComplexSelector<'s> = (CompoundSelector<'s>, Vec<(&'s str, CompoundSelector<'s>)>);
pub fn css_selector_complex(input: &str) -> IResult<&str, ComplexSelector> {
    tuple((
        css_selector_compound,
        many0(tuple((css_selector_combinator, css_selector_compound))),
    ))(input)
}

pub type SelectorList<'s> = Vec<ComplexSelector<'s>>;
#[rustfmt::skip]
pub fn css_selector_list(input: &str) -> IResult<&str, SelectorList> {
    separated_list1(
        tuple((opt(css_space), tag(";"), opt(css_space))),
        css_selector_complex,
    )(input)
}

pub type Declaration<'s> = (&'s str, &'s str);
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
                css_big_token(css_ident),
                css_big_token(move |i| tag(":")(i)),
                recognize(many_till(anychar, tuple((opt(css_space), peek(one_of(";}"))))),
            )),
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
pub fn css_big_token<'i, O: 'i>(parse: impl FnMut(&'i str) -> IResult<&str, O> + Copy) -> impl FnMut(&'i str) -> IResult<&str, O> {
    move |input| terminated(
        preceded(
            tuple((opt(css_space), opt(css_comment), opt(css_space))),
            parse,
        ),
        tuple((opt(css_space), opt(css_comment), opt(css_space))),
    )(input)
}

pub type RuleList<'s> = Vec<Rule<'s>>;
pub fn css_file(input: &str) -> IResult<&str, RuleList> {
    many0(css_big_token(css_rule))(input)
}

#[test]
#[rustfmt::skip]
fn test_css_file() {
    assert_eq!(css_ident("x{}"), Ok(("{}", "x")));
    assert_eq!(css_selector("x{}"), Ok(("{}", "x")));
    assert_eq!(css_selector_compound("x{}"), Ok(("{}", vec!["x"])));
    assert_eq!(css_selector_complex("x{}"), Ok(("{}", (vec!["x"], vec![]))));
    assert_eq!(css_selector_list("x{}"), Ok(("{}", vec![(vec!["x"], vec![])])));
    assert_eq!(css_rule("x{}"), Ok(("", (vec![(vec!["x"], vec![])], vec![]))));
    assert_eq!(css_file("x{}"), Ok(("", vec![(vec![(vec!["x"], vec![])], vec![])])));
}
