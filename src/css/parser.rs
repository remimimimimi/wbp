use cssparser::*;
use log::error;

use crate::css::props::Props;

use super::super::html::Selector;

use super::props::{PropIndex, PropUnion};

pub struct Rule {
    pub selectors: Selector,
    pub important_declarations: Props,
    pub declarations: Props,
}

// #[derive(Clone, Debug)]
pub struct Declaration {
    idx: PropIndex,
    value: PropUnion,
    important: bool,
}

pub struct DeclParser;
pub struct RuleParser;

impl<'i> DeclarationParser<'i> for DeclParser {
    type Declaration = Declaration;
    type Error = CowRcStr<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Declaration, ParseError<'i, CowRcStr<'i>>> {
        let location = input.current_source_location();

        let mut value = vec![];
        let mut important = false;

        loop {
            let start = input.state();
            if let Ok(mut token) = input.next().cloned() {
                // Hack to deal with css-parsing-tests assuming that
                // `!important` in the middle of a declaration value is OK.
                // This can never happen per spec
                // (even CSS Variables forbid top-level `!`)
                if token == Token::Delim('!') {
                    input.reset(&start);
                    if parse_important(input).is_ok() && input.is_exhausted() {
                        important = true;
                        break;
                    }
                    input.reset(&start);
                    token = input.next().unwrap().clone();
                }
                value.push(token.to_css_string());
            } else {
                break;
            }
        }

        let value = value.join(" ");

        let mut dinput = ParserInput::new(&value);
        let mut parser = Parser::new(&mut dinput);

        let (idx, value) = PropUnion::parse(&*name, &mut parser).map_err(move |_| ParseError {
            kind: ParseErrorKind::Custom(name),
            location,
        })?;

        Ok(Declaration {
            idx,
            value,
            important,
        })
    }
}

impl<'i> AtRuleParser<'i> for DeclParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = CowRcStr<'i>;
}

impl<'i> QualifiedRuleParser<'i> for DeclParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = CowRcStr<'i>;
}

// Extend this when we will need at rules support.
impl AtRuleParser<'_> for RuleParser {
    type Prelude = ();
    type AtRule = Rule;
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, Declaration, CowRcStr<'i>> for DeclParser {
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = Selector;
    type QualifiedRule = Rule;
    type Error = ();

    // Selectors parsing
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Selector, ParseError<'i, ()>> {
        let start_pos = input.position();
        while let Ok(_tok) = input.next() {
            //
        }
        let end_pos = input.position();

        let selectors = input.slice(start_pos..end_pos).trim();

        Selector::parse(selectors).map_err(|_| ParseError {
            kind: ParseErrorKind::Custom(()),
            location: input.current_source_location(),
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Selector,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Rule, ParseError<'i, ()>> {
        let mut important_props = Props::new();
        let mut props = Props::new();

        for item in RuleBodyParser::new(input, &mut DeclParser) {
            match item {
                Ok(decl) => unsafe {
                    if decl.important {
                        important_props.set_idx(decl.idx, decl.value);
                    } else {
                        props.set_idx(decl.idx, decl.value);
                    }
                },
                Err(err) => {
                    error!("Error parsing declaration: {:#?}", err);
                }
            }
        }

        Ok(Rule {
            selectors: prelude,
            declarations: props,
            important_declarations: important_props,
        })
    }
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = RuleParser;

    StyleSheetParser::new(&mut parser, &mut rule_parser)
        .filter_map(Result::ok)
        .collect()
}

// #[test]
// fn css_parser_test() {
//     let css = r#"
//         h1, h2 { color: blue; font-size: 12px; }
//         .foo { background: #ff0; margin: 0 auto; }
//     "#;

//     let rules = parse_stylesheet(css);
//     for rule in rules {
//         println!("{:#?}", rule);
//     }
// }
