use crate::{chunk::{self, Chunk, OpCode}, scanner::{self, Scanner, Token, TokenType}, value::Value};
use std::{any::Any, f64, io::Write, thread::current};

pub struct Parser<'a> {
    current: Token<'a>,
    previous: Token<'a>,
    scanner: Box<Scanner<'a>>,
    has_error: bool,
    panic_mode: bool,
    chunk: Box<Chunk>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum Precedence {
    None,
    Assignment, // =
    Or,         // ||
    And,        // &&
    Equality,   // ==
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl From<u8> for Precedence {
    fn from(value: u8) -> Self {
        match value {
            0 => Precedence::None,
            1 => Precedence::Assignment,
            2 => Precedence::Or,
            3 => Precedence::And,
            4 => Precedence::Equality,
            5 => Precedence::Comparison,
            6 => Precedence::Term,
            7 => Precedence::Factor,
            8 => Precedence::Unary,
            9 => Precedence::Call,
            10 => Precedence::Primary,
            _ => unreachable!("Invalid precedence value: {}", value),
        }
    }
}

//type ParserFn = for<'a> fn(&'a mut Parser<'a>);
type ParserFn = fn(&mut Parser<'_>);

#[derive(Debug, Clone, Copy)]
struct ParseRule {
    prefix: Option<ParserFn>,
    infix: Option<ParserFn>,
    precedence: Precedence,
}

impl ParseRule {
    const fn new(
        prefix: Option<ParserFn>,
        infix: Option<ParserFn>,
        precedence: Precedence,
    ) -> Self {
        ParseRule {
            prefix,
            infix,
            precedence,
        }
    }
}

const RULES: [ParseRule; TokenType::Eof as usize + 1] = {
    let mut rules = [ParseRule::new(None, None, Precedence::None); TokenType::Eof as usize + 1];
    rules[TokenType::LeftParen as usize] = ParseRule::new(Some(|parser| parser.grouping()), None, Precedence::None);
    rules[TokenType::Minus as usize] = ParseRule::new(Some(|parser| parser.unary()), Some(|parser| parser.binary()), Precedence::Term);
    rules[TokenType::Plus as usize] = ParseRule::new(None, Some(|parser| parser.binary()), Precedence::Term);
    rules[TokenType::Slash as usize] = ParseRule::new(None, Some(|parser| parser.binary()), Precedence::Factor);
    rules[TokenType::Star as usize] = ParseRule::new(None, Some(|parser| parser.binary()), Precedence::Factor);
    rules[TokenType::Number as usize] = ParseRule::new(Some(|parser| parser.number()), None, Precedence::None);

    rules
};

impl<'a> Parser<'a> {
    pub fn new(scanner: Box<Scanner<'a>>) -> Box<Parser<'a>> {
        Box::new(Parser{
            current: Token{token_type: TokenType::Eof, value: "", line: 0},
            previous: Token{token_type: TokenType::Eof, value: "", line: 0},
            scanner,
            has_error: false,
            panic_mode: false,
            chunk: Chunk::new(),
        })
    }

    pub fn compile(&mut self, source: &'a str, chunk: Box<Chunk>) -> bool {
        self.chunk = chunk;
        self.advance();

        // expression();
        //consumer();
        
        self.end_compiler();
        return !self.has_error;
    }

    fn advance(&mut self) {
        self.previous = self.current.clone();
        loop {
            self.current = self.scanner.scan_token();
            if self.current.token_type != TokenType::Error {
                break;
            }

            self.error_at_current(self.current.value);
        }
    }

    fn consume(&mut self, token_type: TokenType, message: &'a str) {
        if self.current.token_type == token_type {
            self.advance();
            return;
        }

        self.error_at_current(message);
    }

    fn emit_byte(&mut self, byte: u8) {
        let line = self.previous.line;
        self.current_chunk().write(byte, line);
    }

    fn emit_bytes(&mut self, byte1: u8, byte2: u8) {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn emit_constant(&mut self, value: Value) {
        let byte = self.make_constant(value);
        self.emit_bytes(OpCode::Constant.to_byte(), byte);
    }

    fn emit_return(&mut self) {
        self.emit_byte(chunk::OpCode::Return.to_byte());
    }

    fn end_compiler(&mut self) {
        self.emit_return();

        debug_feature::disassemble_chunk(self);
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::max_value().into() {
            self.error("Too many constants in one chunk.");
            return 0;
        }
        constant as u8
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.chunk
    }

    fn number(&mut self) {
        let value = match self.previous.value.parse::<f64>() {
            Ok(num) => num,
            Err(_) => 0.0,
        };
        self.emit_constant(value);
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn unary(&mut self) {
        let operator_type = self.previous.token_type;

        self.parse_precedence(Precedence::Unary);

        match operator_type {
            TokenType::Minus => self.emit_byte(OpCode::Negate.to_byte()),
            _ => unreachable!("Expect '-' operator."),
        }
    }

    fn binary(&mut self) {
        let operator_type = self.previous.token_type;
        let rule = &RULES[operator_type as usize];
        self.parse_precedence((rule.precedence as u8 + 1).into());

        match operator_type {
            TokenType::Plus => self.emit_byte(OpCode::Add.to_byte()),
            TokenType::Minus => self.emit_byte(OpCode::Subtract.to_byte()),
            TokenType::Star => self.emit_byte(OpCode::Multiply.to_byte()),
            TokenType::Slash => self.emit_byte(OpCode::Divide.to_byte()),
            _ => unreachable!("Unexpected binary operator: {}", operator_type)
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        if let Some(prefix) = &RULES[self.previous.token_type as usize].prefix {
            prefix(self);
        } else {
            self.error("Expect expression.");
            return;
        }

        while precedence as u8 <= (&RULES[self.current.token_type as usize]).precedence as u8 {
            self.advance();
            if let Some(infix) = &RULES[self.previous.token_type as usize].infix {
                infix(self);
            } else {
                self.error("Expect infix parse function.");
                return;
            }
        }
    }

    fn error(&mut self, message: &'a str) {
        self.error_at(&self.previous.clone(), message);
    }

    fn error_at_current(&mut self, message: &'a str) {
        self.error_at(&self.current.clone(), message);
    }

    fn error_at(&mut self, token: &Token, message: &'a str) {
        if self.panic_mode {
            return;
        }

        self.panic_mode = true;
        write!(&mut std::io::stderr(), "[line {}] Error", token.line).expect("Failed to write to stderr");

        match token.token_type {
            TokenType::Eof => write!(&mut std::io::stderr(), " at end").expect("Failed to write to stderr"),
            TokenType::Error => {},
            _ => write!(&mut std::io::stderr(), " at '{}'", token.value).expect("Failed to write to stderr"),
        };

        writeln!(&mut std::io::stderr(), ": {}", message).expect("Failed to write to stderr");
        self.has_error = true;
    }
}

#[cfg(feature = "debug_print_code")]
mod debug_feature {
    use crate::debug;

    use super::*;

    pub fn disassemble_chunk(parser: &mut Parser) {
        if !parser.has_error {
            debug::disassemble_chunk(parser.current_chunk(), "code");
        }
    }
}

#[cfg(not(feature = "debug_print_code"))]
mod debug_feature {
    use super::*;

    pub fn disassemble_chunk(parser: &Parser) {}
}