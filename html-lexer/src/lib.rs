use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::char,
    combinator::{fail, map, map_parser, opt},
    multi::many0,
    sequence::{delimited, preceded, tuple},
    IResult, Needed,
};

include!(concat!(env!("OUT_DIR"), "/entities.rs"));

pub fn is_html_space(c: char) -> bool {
    c.is_ascii_whitespace()
}

pub fn html_ident(input: &str) -> IResult<&str, &str> {
    take_while1(|c| match c {
        c if c.is_ascii_alphanumeric() => true,
        '?' | ':' | '-' => true,
        _ => false,
    })(input)
}

pub fn html_space(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_ascii_whitespace())(input)
}

fn quoted_attr_value(mut input: &str) -> IResult<&str, String> {
    let mut result = String::new();

    while !input.is_empty() {
        let (rest, text) = html_text(true)(input).expect("parser is infallible");
        result += text;
        input = rest;
    }

    Ok((input, result))
}

pub fn html_attr_value(input: &str) -> IResult<&str, String> {
    alt((
        delimited(
            char('"'),
            map_parser(take_while(|c| c != '"'), quoted_attr_value),
            char('"'),
        ),
        delimited(
            char('\''),
            map_parser(take_while(|c| c != '\''), quoted_attr_value),
            char('\''),
        ),
        map(
            take_while(|c| match c {
                c if is_html_space(c) => false,
                '"' | '\'' | '=' | '<' | '>' | '`' => false,
                _ => true,
            }),
            |x: &str| x.to_owned(),
        ),
    ))(input)
}

pub fn html_attr(input: &str) -> IResult<&str, (&str, String)> {
    let (rest, (_, name, value)) = tuple((
        html_space,
        html_ident,
        opt(preceded(
            tuple((opt(html_space), tag("="), opt(html_space))),
            html_attr_value,
        )),
    ))(input)?;

    Ok((rest, (name, value.unwrap_or("".to_owned()))))
}

pub fn html_tag(input: &str) -> IResult<&str, (bool, &str, Vec<(&str, String)>)> {
    let (rest, (slash, name, attrs, _)) = delimited(
        char('<'),
        tuple((opt(tag("/")), html_ident, many0(html_attr), opt(html_space))),
        tuple((opt(tag("/")), char('>'))),
    )(input)?;

    Ok((rest, (slash.is_some(), name, attrs)))
}

pub fn shortest_until_tag_no_case(tag: &str) -> impl FnMut(&str) -> IResult<&str, &str> + '_ {
    |input: &str| {
        let Some(index) = input.to_ascii_lowercase().find(&tag.to_ascii_lowercase())
            else { return Err(nom::Err::Incomplete(Needed::Unknown)) };

        Ok((&input[index + tag.len()..], &input[..index]))
    }
}

pub fn html_script(input: &str) -> IResult<&str, (Vec<(&str, String)>, &str)> {
    let (rest, (attrs, _, _, text)) = preceded(
        tag_no_case("<script"),
        tuple((
            many0(html_attr),
            opt(html_space),
            tag(">"),
            shortest_until_tag_no_case("</script>"),
        )),
    )(input)?;

    Ok((rest, (attrs, text)))
}

pub fn html_style(input: &str) -> IResult<&str, (Vec<(&str, String)>, &str)> {
    let (rest, (attrs, _, _, text)) = preceded(
        tag_no_case("<style"),
        tuple((
            many0(html_attr),
            opt(html_space),
            tag(">"),
            shortest_until_tag_no_case("</style>"),
        )),
    )(input)?;

    Ok((rest, (attrs, text)))
}

pub fn html_comment(input: &str) -> IResult<&str, &str> {
    preceded(tag("<!--"), shortest_until_tag_no_case("-->"))(input)
}

pub fn html_doctype(input: &str) -> IResult<&str, &str> {
    preceded(tag("<!"), shortest_until_tag_no_case(">"))(input)
}

pub fn html_entity(in_attr: bool) -> impl FnMut(&str) -> IResult<&str, &str> {
    move |input: &str| {
        for i in ENTITIES_WITH_SEMICOLON_REGEX.matches(input) {
            let (name, value) = ENTITIES_WITH_SEMICOLON[i];
            return Ok((input.strip_prefix(name).unwrap(), value));
        }
        for i in ENTITIES_WITHOUT_SEMICOLON_REGEX.matches(input) {
            let (name, value) = ENTITIES_WITHOUT_SEMICOLON[i];
            let rest = input.strip_prefix(name).unwrap();
            if in_attr && rest.starts_with(|c: char| c == '=' || c.is_ascii_alphanumeric()) {
                return Ok((rest, name));
            } else {
                return Ok((rest, value));
            }
        }

        if input.is_empty() {
            fail(input)
        } else {
            let (ampersand, rest) = input.split_at(1);
            Ok((rest, ampersand))
        }
    }
}

pub fn html_text(in_attr: bool) -> impl FnMut(&str) -> IResult<&str, &str> {
    move |input: &str| {
        alt((
            take_while1(|c| c != '<' && c != '&'),
            html_entity(in_attr),
            tag("<"),
        ))(input)
    }
}

#[derive(Debug)]
pub enum HtmlToken<'i> {
    Comment(&'i str),
    Script(Vec<(&'i str, String)>, &'i str),
    Style(Vec<(&'i str, String)>, &'i str),
    Tag(bool, &'i str, Vec<(&'i str, String)>),
    Text(&'i str),
    Doctype(&'i str),
}

pub fn html_token(input: &str) -> IResult<&str, HtmlToken> {
    if let Ok((rest, text)) = html_comment(input) {
        return Ok((rest, HtmlToken::Comment(text)));
    }
    if let Ok((rest, (attrs, text))) = html_script(input) {
        return Ok((rest, HtmlToken::Script(attrs, text)));
    }
    if let Ok((rest, (attrs, text))) = html_style(input) {
        return Ok((rest, HtmlToken::Style(attrs, text)));
    }
    if let Ok((rest, (closing, name, attrs))) = html_tag(input) {
        return Ok((rest, HtmlToken::Tag(closing, name, attrs)));
    }
    if let Ok((rest, doctype)) = html_doctype(input) {
        return Ok((rest, HtmlToken::Doctype(doctype)));
    }

    let (rest, text) = html_text(false)(input)?;

    Ok((rest, HtmlToken::Text(text)))
}

#[derive(Debug)]
pub enum HtmlWord<'i> {
    Space(&'i str),
    Other(&'i str),
}

pub fn html_word(input: &str) -> IResult<&str, HtmlWord> {
    if let Ok((rest, text)) = html_space(input) {
        return Ok((rest, HtmlWord::Space(text)));
    }

    let (rest, text) = take_while1(|c: char| !c.is_ascii_whitespace())(input)?;

    Ok((rest, HtmlWord::Other(text)))
}
