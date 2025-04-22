//! Parser for the CSS Value Defintion Syntax. Thorough description can be found at [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Values_and_Units/Value_definition_syntax#component_value_multipliers), but this parser implement only subset from [CSS 2.1 specification](https://www.w3.org/TR/2011/REC-CSS2-20110607/propidx.html#q24.0).

use chumsky::prelude::*;

// AST definition carrying &str slices
#[derive(Debug, Clone, PartialEq)]
pub enum VDS<'a> {
    /// Just a keyword, regular ident.
    Keyword(&'a str),
    /// Some built-in values, have syntax `<some-ident>`.
    Value(&'a str),
    /// Already defined property, have syntax `'some-property'`.
    Type(&'a str),

    ZeroOrMore(Box<VDS<'a>>),
    OneOrMore(Box<VDS<'a>>),
    Optional(Box<VDS<'a>>),
    Range(Box<VDS<'a>>, usize, usize),

    Sequence(Vec<VDS<'a>>),
    Choice(Vec<VDS<'a>>),
    AllOf(Vec<VDS<'a>>),
    OneOrMoreOf(Vec<VDS<'a>>),
}

impl VDS<'_> {
    pub fn as_ext_ty(&self) -> Option<&str> {
        match self {
            VDS::Keyword(s) | VDS::Value(s) | VDS::Type(s) => Some(*s),
            _ => None,
        }
    }
}

pub fn css_value_parser<'a>() -> impl Parser<'a, &'a str, VDS<'a>> {
    let ident = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '-')
        .repeated()
        .at_least(1)
        .to_slice()
        .padded();

    // Parse keywords: sequences of letters or hyphens
    let keyword = ident.clone().map(VDS::Keyword);

    // Parse <value> placeholders
    let value = ident
        .clone()
        .map(VDS::Value)
        .delimited_by(just('<').padded(), just('>').padded());

    // Parse <type> placeholders.
    let type_parser = ident
        .clone()
        .map(VDS::Type)
        .delimited_by(just('\'').padded(), just('\'').padded());

    recursive(|expr| {
        // Grouping with [ ... ]
        let group = expr
            .clone()
            .delimited_by(just('[').padded(), just(']').padded());

        let base = choice((group, value.clone(), type_parser.clone(), keyword.clone()));

        // Multipliers: *, +, ?, or {n,m}
        let mult = choice((
            just('*').to((0, None)).padded(),
            just('+').to((1, None)).padded(),
            just('?').to((0, Some(1))).padded(),
            // explicit ranges like {n,m}
            text::int(10)
                .from_str::<usize>()
                .unwrapped()
                .padded()
                .then_ignore(just(',').padded())
                .then(text::int(10).from_str::<usize>().unwrapped())
                .map(|(min, max)| (min, Some(max)))
                .delimited_by(just('{').padded(), just('}').padded()),
        ))
        .boxed();

        // Apply optional multiplier
        let primary = base
            .clone()
            .then(mult.or_not())
            .map(|(node, opt)| match opt {
                None => node,
                Some((0, None)) => VDS::ZeroOrMore(Box::new(node)),
                Some((1, None)) => VDS::OneOrMore(Box::new(node)),
                Some((0, Some(1))) => VDS::Optional(Box::new(node)),
                Some((min, Some(max))) => VDS::Range(Box::new(node), min, max),
                Some((min, None)) => VDS::Range(Box::new(node), min, min),
            });

        // Sequence (juxtaposition)
        let seq = primary.repeated().at_least(1).collect::<Vec<_>>().map(|v| {
            if v.len() == 1 {
                v.into_iter().next().unwrap()
            } else {
                VDS::Sequence(v)
            }
        });

        // && (all required, any order)
        let all_of = seq
            .clone()
            .separated_by(just("&&").padded())
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|v| {
                if v.len() == 1 {
                    v.into_iter().next().unwrap()
                } else {
                    VDS::AllOf(v)
                }
            });

        // || (one or more, any order)
        let one_of = all_of
            .clone()
            .separated_by(just("||").padded())
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|v| {
                if v.len() == 1 {
                    v.into_iter().next().unwrap()
                } else {
                    VDS::OneOrMoreOf(v)
                }
            });

        // | (exclusive choice)
        one_of
            .separated_by(just('|').padded())
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|v| {
                if v.len() == 1 {
                    v.into_iter().next().unwrap()
                } else {
                    VDS::Choice(v)
                }
            })
    })
    .padded()
    .then_ignore(end())
}

/// TODO: Write more tests.
#[cfg(test)]
mod tests {
    use super::*;
    use VDS::*;

    #[test]
    fn simple_test() {
        let input = "<length> | <percentage> | auto";
        let ast = css_value_parser().parse(input).unwrap();

        assert_eq!(
            ast,
            Choice(vec![Type("length"), Type("percentage"), Keyword("auto")])
        );
    }
}
