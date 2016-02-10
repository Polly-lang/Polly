use std::collections::HashMap;
use std::iter::Peekable;
use std::vec::IntoIter;

use super::lexer::Lexer;
use super::tokens::*;
use super::tokens::AstError::*;
use super::tokens::Lexeme::*;
use super::tokens::Operator::*;
use super::tokens::Token::*;
use template::{PolyFn};

/// Shortens Result<T, AstError> to Result<T>.
pub type AstResult = Result<Token, AstError>;

macro_rules! unexpected_eof {
    ($token:expr) => {
        return Err(UnexpectedEof($token));
    }
}
macro_rules! get_identifer {
    ($token:expr, $index:expr, $unexpected:expr) => {
        match $token {
            Some(Word(_, text)) => text,
            Some(unexpected_token) => {
                return Err($unexpected(unexpected_token))
            }
            None => return Err(UnexpectedEof(Symbol($index, At))),
        };
    }
}


macro_rules! get_children {
    ($token:expr, $parent:expr) => 
    {{
        let mut depth: usize = 0;
        let mut open_brace_index: usize = 0;
        let mut close_brace_index: usize = 0;
        let mut children = Vec::new();
        while let Some(token) = $token {
            match token {
                Symbol(index, OpenBrace) => {
                    depth += 1;

                    if depth != 0 {
                        children.push(Symbol(index, OpenBrace));
                    }
                    open_brace_index = index;
                }
                Symbol(index, CloseBrace) => {
                    if depth == 0 {
                        break;
                    } else {
                        depth -= 1;
                        children.push(Symbol(index, CloseBrace));
                    }
                    close_brace_index = index;
                }
                t => children.push(t),
            }
        }

        if depth > 0 {
            return Err(UnclosedOpenBraces(open_brace_index));
        } else if depth != 0 {
            return Err(UnclosedOpenBraces(close_brace_index));
        }
        if !children.is_empty() {
            $parent.add_children(&mut Parser::new(children).output());
        }
    }}
}

/// The struct detailing the parser itself.
pub struct Parser {
    input: Peekable<IntoIter<Lexeme>>,
    output: Vec<AstResult>,
    components: HashMap<String, Component>
}

impl Parser {
    /// Generates Parser from Lexer
    pub fn new(lexemes: Vec<Lexeme>) -> Self {
        let mut parser = Parser { input: lexemes.into_iter().peekable(), output: Vec::new(), components: HashMap::new(),}; 
        loop {
            match parser.parse_token() {
                Err(Eof) => break,
                token => parser.push(token),
            }
        }
        parser
    }

    fn push(&mut self, token: AstResult) {
        self.output.push(token);
    }

    fn take(&mut self) -> Option<Lexeme> {
        self.input.next()
    }

    fn peek(&mut self) -> Option<Lexeme> {
        match self.input.peek() {
            Some(token) => Some(token.clone()),
            None => None,
        }
    }
    /// Output result vector
    pub fn output(&self) -> Vec<AstResult> {
        // Need to figure out a way to not clone the vector
        self.output.to_vec()
    }

    pub fn get_components(&self) -> HashMap<String, Component> {
        self.components.clone()
    }

    fn read_leading_quotes(&mut self) -> String {
        let mut value = String::new();
        while let Some(token) = self.take() {
            match token {
                Symbol(_, Quote) => break,
                Word(_, text) => value.push_str(&*text),
                Symbol(_, operator) => value.push_str(&*operator.to_string()),
                Empty => {}
            }
        }
        value
    }

    fn parse_component(&mut self, allow_definition: bool, index: usize) -> AstResult {
        let name = get_identifer!(self.take(), index, InvalidComponent);
        let mut component = Component::new(name);

        while let Some(token) = self.peek() {
            match token {
                Symbol(_, OpenParam) => {
                    let _ = self.take();
                    while let Some(token) = self.take() {
                        match token {
                            Symbol(index, At) => {

                                let identifier = get_identifer!(self.take(),
                                                                index,
                                                                UnexpectedToken);
                                component.add_arg_value(identifier);
                            }
                            Symbol(_, CloseParam) => {
                                match self.peek() {
                                    Some(Symbol(_, OpenBrace)) => break,
                                    _ => {
                                        return Ok(CompCall((ComponentCall::from_component(component))));
                                    }
                                }
                            }
                            Symbol(_, Comma) => {}
                            unexpected_token => return Err(UnexpectedToken(unexpected_token)),
                        }
                    }
                }
                token @ Symbol(_, OpenBrace) => {
                    let _ = self.take();
                    if allow_definition {
                        get_children!(self.take(), component);
                        break;
                    } else {
                        return Err(ExpectedCompCall(token));
                    }
                }
                unexpected_token => return Err(UnexpectedToken(unexpected_token)),
            }
        }
        if allow_definition {
            self.components.insert(component.name().clone(), component);
            Ok(Text(String::new()))
        } else {
            // This unreachable, because with allow_definition = false, we should either get a
            // CompCall, or a ExpectedCompCall error.
            unreachable!()
        }
    }

    fn parse_element(&mut self, index: usize) -> AstResult {
        let tag = get_identifer!(self.take(), index, InvalidElement);
        let mut element = Element::new(tag.trim().to_owned());

        'element: while let Some(token) = self.take() {
            match token {
                Symbol(index, Ampersand) => {
                    let identifier = get_identifer!(self.take(), index, ExpectedCompCall);
                    let mut component_call = ComponentCall::new(identifier);

                    if let Some(Symbol(_, OpenParam)) = self.peek() {
                        let _ = self.take();
                        while let Some(symbol) = self.take() {
                            match symbol {
                                Symbol(_, CloseParam) => break,
                                Symbol(index, At) => {
                                    let identifier = get_identifer!(self.take(),
                                                                    index,
                                                                    ExpectedVariable);
                                    component_call.add_value(identifier);
                                }
                                Symbol(_, Comma) => {}
                                unexpected_token => return Err(UnexpectedToken(unexpected_token)),
                            }
                        }
                    }
                    element.add_resource(component_call)
                }
                Symbol(index, OpenParam) => {
                    while let Some(token) = self.take() {
                        match token {
                            Symbol(_, CloseParam) => {
                                match self.peek() {
                                    Some(Symbol(_, OpenBrace)) => break,
                                    _ => return Ok(Html(element)),
                                }
                            }
                            Symbol(_, Quote) => {
                                let key = format!("{}{}{}", '"', self.read_leading_quotes(), '"');
                                element.add_attribute(key, String::from(""));
                            }
                            Word(_, key) => {
                                let value = match self.peek() {
                                    Some(Symbol(index, Equals)) => {
                                        let _ = self.take();
                                        match self.take() {
                                            Some(Word(_, text)) => text,
                                            Some(Symbol(_, Quote)) => self.read_leading_quotes(),
                                            Some(unexpected_token) => {
                                                return Err(InvalidTokenInAttributes(unexpected_token));
                                            }
                                            None => {
                                                return Err(UnexpectedEof(Symbol(index, Equals)));
                                            }
                                        }
                                    }
                                    Some(Word(_, _)) => String::from(""),
                                    Some(Symbol(_, CloseParam)) => String::from(""),
                                    Some(Symbol(_, Quote)) => String::from(""),
                                    Some(invalid_token) => {
                                        return Err(InvalidTokenInAttributes(invalid_token))
                                    }
                                    None => return Err(UnexpectedEof(Word(index, key))),
                                };

                                element.add_attribute(key, value);
                            }
                            invalid_token => return Err(InvalidTokenInAttributes(invalid_token)),
                        }
                    }
                }
                Symbol(index, Dot) => {
                    match self.take() {
                        Some(Word(_, class)) => element.add_class(class),
                        Some(unexpected_token) => {
                            return Err(NoNameAttachedToClass(unexpected_token))
                        }
                        None => return Err(UnexpectedEof(Symbol(index, Dot))),
                    }
                }
                Symbol(index, Pound) => {
                    match self.take() {
                        Some(Word(_, id)) => element.add_attribute(String::from("id"), id),
                        Some(unexpected_token) => return Err(NoNameAttachedToId(unexpected_token)),
                        None => return Err(UnexpectedEof(Symbol(index, Pound))),
                    }
                }
                Symbol(_, OpenBrace) => {
                    get_children!(self.take(), element);
                    break;
                }
                Word(_, text) => element.add_text(text),

                unexpected_token => return Err(UnexpectedToken(unexpected_token)),
            }
        }
        Ok(Html(element))
    }

    fn parse_variable(&mut self, index: usize) -> AstResult {
        // If we find a variable, we expect a word after it.
        match self.take() {
            Some(Word(index, text)) => {
                let mut new_text = text.clone();
                while let Some(Symbol(_, Dot)) = self.peek() {
                    let _ = self.take();
                    new_text.push('.');

                    match self.take() {
                        Some(Word(_, member)) => new_text.push_str(&*member),
                        Some(unexpected_token) => return Err(ExpectedVariable(unexpected_token)),
                        None => return Err(UnexpectedEof(Symbol(index, Dot))),
                    }
                }
                Ok(Variable(new_text))
            }
            Some(unexpected_token) => Err(ExpectedVariable(unexpected_token)),
            None => Err(UnexpectedEof(Symbol(index, At))),
        }
    }

    fn parse_token(&mut self) -> AstResult {

        match self.take() {
            // concatenate all the word tokens that are adjacent to each other into a single "Text"
            // token.
            Some(Word(_, word)) => {
                let mut text = String::from(word);
                loop {
                    let peek = self.peek();
                    match peek {
                        Some(Word(_, ref peek_text)) => {
                            text.push_str(&*peek_text);
                            let _ = self.take();
                        }
                        _ => return Ok(Text(text)),
                    }
                }
            }
            Some(Symbol(index, At)) => self.parse_variable(index),
            Some(Symbol(index, ForwardSlash)) => self.parse_element(index),
            Some(Symbol(_, BackSlash)) => {
                match self.peek() {
                    Some(Symbol(_, ref operator)) => {
                        let _ = self.take();
                        Ok(Text(operator.to_string()))
                    }
                    Some(_) => Ok(Text(String::new())),
                    None => Err(Eof),
                }
            }
            Some(Symbol(index, Ampersand)) => self.parse_component(true, index),
            Some(Symbol(index, Dollar)) => {
                let identifier = get_identifer!(self.take(), index, InvalidFunctionCall);
                let mut func_call = FunctionCall::new(identifier);

                match self.take() {
                    Some(Symbol(_, OpenParam)) => {
                        while let Some(token) = self.take() {
                            match token {
                                Word(index, arg_name) => {
                                    match self.take() {
                                        Some(Symbol(index, Equals)) => {
                                            match self.take() {
                                                Some(Symbol(index, At)) => {
                                                    match self.take() {
                                                        Some(Word(_, identifier)) => {
                                                            func_call.add_value_arg(arg_name,
                                                                                    identifier);
                                                        }
                                                        Some(unexpected_token) => return Err(ExpectedVariable(unexpected_token)),
                                                        None => unexpected_eof!(Symbol(index, At)),
                                                    }
                                                }
                                                Some(Symbol(index, Ampersand)) => {
                                                    match self.take() {
                                                        Some(Word(_, identifier)) => {
                                                            func_call.add_component_arg(arg_name,
                                                                                        identifier);
                                                        }
                                                        Some(unexpected_token) => return Err(ExpectedCompCall(unexpected_token)),
                                                        None => unexpected_eof!(Symbol(index, Ampersand)),
                                                    }
                                                }
                                                Some(unexpected_token) => return Err(UnexpectedToken(unexpected_token)),
                                                None => unexpected_eof!(Symbol(index, Equals)),
                                                
                                            }
                                        }
                                        Some(unexpected_token) => {
                                            return Err(InvalidFunctionCall(unexpected_token))
                                        }
                                        None => unexpected_eof!(Word(index, arg_name)),

                                    }
                                }
                                Symbol(_, CloseParam) => break,
                                Symbol(_, Comma) => {},
                                unexpected_token => return Err(UnexpectedToken(unexpected_token)),
                            }
                        }
                    }
                    Some(unexpected_token) => return Err(InvalidFunctionCall(unexpected_token)),
                    None => return Err(UnexpectedEof(Symbol(index, Dollar))),
                }
                Ok(Function(func_call))
            }
            Some(Symbol(_, operator)) => Ok(Text(operator.to_string())),
            Some(Empty) => unreachable!(),
            None => Err(Eof),
        }
    }
}
