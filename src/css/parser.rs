use cssparser::*;

#[derive(Clone, Debug)]
pub struct Rule {
    pub selectors: Vec<String>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, Debug)]
pub struct Declaration {
    pub name: String,
    pub value: Vec<String>,
    pub important: bool,
}

pub struct DeclParser;
pub struct RuleParser;

impl<'i> DeclarationParser<'i> for DeclParser {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Declaration, ParseError<'i, ()>> {
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

        Ok(Declaration {
            name: name.to_string(),
            value,
            important,
        })
    }
}

impl<'i> AtRuleParser<'i> for DeclParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for DeclParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

// Extend this when we will need at rules support.
impl<'i> AtRuleParser<'i> for RuleParser {
    type Prelude = ();
    type AtRule = Rule;
    type Error = ();
}

impl RuleBodyItemParser<'_, Declaration, ()> for DeclParser {
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = Vec<String>;
    type QualifiedRule = Rule;
    type Error = ();

    // Selectors parsing
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Vec<String>, ParseError<'i, ()>> {
        input.parse_comma_separated(|input| {
            let start_pos = input.position();
            while let Ok(_tok) = input.next() {
                //
            }
            let end_pos = input.position();

            Ok(input.slice(start_pos..end_pos).trim().into())
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Vec<String>,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Rule, ParseError<'i, ()>> {
        let mut declarations = vec![];

        for item in RuleBodyParser::new(input, &mut DeclParser) {
            if let Ok(decl) = item {
                declarations.push(decl.clone());
            } else {
                eprintln!("Error parsing declaration: {:#?}", item);
            }
        }

        Ok(Rule {
            selectors: prelude,
            declarations,
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

#[test]
fn css_parser_test() {
    let css = r#"
        h1, h2 { color: blue; font-size: 12px; }
        .foo { background: #ff0; margin: 0 auto; }
    "#;

    let rules = parse_stylesheet(css);
    for rule in rules {
        println!("{:#?}", rule);
    }
}
