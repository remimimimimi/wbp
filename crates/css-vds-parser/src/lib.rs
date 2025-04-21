//! Parser for the CSS Value Defintion Syntax. Thorough description can be found at [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Values_and_Units/Value_definition_syntax#component_value_multipliers), but this parser implement only subset from [CSS 2.1 specification](https://www.w3.org/TR/2011/REC-CSS2-20110607/propidx.html#q24.0).

use chumsky::prelude::*;

// AST definition carrying &str slices
#[derive(Debug, Clone, PartialEq)]
pub enum VDS<'a> {
    Sequence(Vec<VDS<'a>>),
    Choice(Vec<VDS<'a>>),
    AllOf(Vec<VDS<'a>>),
    OneOrMoreOf(Vec<VDS<'a>>),
    Type(&'a str),
    Keyword(&'a str),
    ZeroOrMore(Box<VDS<'a>>),
    OneOrMore(Box<VDS<'a>>),
    Optional(Box<VDS<'a>>),
    Range(Box<VDS<'a>>, usize, usize),
}

pub fn css_value_parser<'a>() -> impl Parser<'a, &'a str, VDS<'a>> {
    // Parse keywords: sequences of letters or hyphens
    let keyword = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '-')
        .repeated()
        .at_least(1)
        .to_slice()
        .padded()
        .map(VDS::Keyword);

    // Parse <type> placeholders, possibly quoted
    let type_ph = just('<')
        .ignore_then(choice((
            just('\'')
                .ignore_then(
                    any()
                        .filter(|c| *c != '\'')
                        .repeated()
                        .at_least(1)
                        .to_slice(),
                )
                .then_ignore(just('\'')),
            any()
                .filter(|c: &char| c.is_alphabetic() || *c == '-')
                .repeated()
                .at_least(1)
                .to_slice(),
        )))
        .then_ignore(just('>'))
        .padded()
        .map(VDS::Type);

    recursive(|expr| {
        // Grouping with [ ... ]
        let group = expr
            .clone()
            .delimited_by(just('[').padded(), just(']').padded());

        let base = choice((group, type_ph.clone(), keyword.clone()));

        // Multipliers: *, +, ?, or {n,m}
        let mult = choice((
            just('*').to((0, None)).padded(),
            just('+').to((1, None)).padded(),
            just('?').to((0, Some(1))).padded(),
            // explicit ranges like {n,m}
            just('{')
                .padded()
                .ignore_then(text::int(10).from_str::<usize>().unwrapped().padded())
                .then_ignore(just(',').padded())
                .then(text::int(10).from_str::<usize>().unwrapped())
                .then_ignore(just('}').padded())
                .map(|(min, max)| (min, Some(max))),
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
