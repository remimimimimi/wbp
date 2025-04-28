use cssparser::*;
use log::error;

use crate::css::props::{PropIndex, PropUnion, Props};
use crate::selector::SelectorGroup;

pub mod props;
pub mod values;

#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: SelectorGroup,
    pub important_declarations: Props,
    pub declarations: Props,
}

// #[derive(Clone, Debug)]
struct Declaration {
    idx: PropIndex,
    value: PropUnion,
    important: bool,
}

struct DeclParser;
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

        let (idx, value) = PropUnion::parse(&name, &mut parser).map_err(move |_| ParseError {
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
    type Prelude = SelectorGroup;
    type QualifiedRule = Rule;
    type Error = ();

    // Selectors parsing
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<SelectorGroup, ParseError<'i, ()>> {
        let start_pos = input.position();
        while let Ok(_tok) = input.next() {
            //
        }
        let end_pos = input.position();

        let selectors = input.slice(start_pos..end_pos).trim();

        SelectorGroup::parse(selectors).map_err(|_| ParseError {
            kind: ParseErrorKind::Custom(()),
            location: input.current_source_location(),
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: SelectorGroup,
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

pub type StyleSheet = Vec<Rule>;

pub fn parse_stylesheet(css: &str) -> StyleSheet {
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
//         // h1, h2 { color: blue; font-size: 12px; }
//         // .foo { background: #ff0; margin: 0 auto; }

// *             {}  /* a=0 b=0 c=0 d=0 -> specificity = 0,0,0,0 */
//  li            {}  /* a=0 b=0 c=0 d=1 -> specificity = 0,0,0,1 */
//  ul li         {}  /* a=0 b=0 c=0 d=2 -> specificity = 0,0,0,2 */
//  ul ol+li      {}  /* a=0 b=0 c=0 d=3 -> specificity = 0,0,0,3 */
//  h1 + *[rel=up]{}  /* a=0 b=0 c=1 d=1 -> specificity = 0,0,1,1 */
//  ul ol li.red  {}  /* a=0 b=0 c=1 d=3 -> specificity = 0,0,1,3 */
//  li.red.level  {}  /* a=0 b=0 c=2 d=1 -> specificity = 0,0,2,1 */
//  #x34y         {}  /* a=0 b=1 c=0 d=0 -> specificity = 0,1,0,0 */
// nav > a:hover::before {}
// ul#primary-nav li.active {}
//     "#;

//     let rules = parse_stylesheet(css);
//     for rule in rules {
//         let selectors = rule.selectors;

//         println!("Selectors: {}", selectors.to_css_string());
//         for selector in selectors.selectors.slice() {
//             let s = selector.specificity();
//             println!(
//                 "  - \"{}\" has specificity (0, {}, {}, {})",
//                 selector.to_css_string(),
//                 (s >> 10 * 2) & 0x3FF,
//                 (s >> 10 * 1) & 0x3FF,
//                 (s >> 10 * 0) & 0x3FF,
//             );
//         }
//         // println!("{:#?}", rule);
//     }
// }
