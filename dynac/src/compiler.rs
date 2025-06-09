use crate::{chunk::{self, Chunk, OpCode}, objects::object::Object, objects::object_manager::{self, ObjectManager}, scanner::{self, Scanner, Token, TokenType}, table::Table, value::{self, *}};
use std::{any::Any, f64, io::Write, thread::current};

pub struct Parser<'a> {
    current: Token<'a>,
    previous: Token<'a>,
    scanner: Option<Box<Scanner<'a>>>,
    has_error: bool,
    panic_mode: bool,
    chunk: Option<&'a mut Chunk>,
    compiler: Compiler<'a>,
    object_manager: &'a mut ObjectManager,
    intern_strings: &'a mut Table,
}

struct Local<'a> {
    name: Token<'a>,
    depth: i32,
}

struct Compiler<'a> {
    locals: Vec<Local<'a>>,
    scope_depth: i32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
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
type ParserFn = fn(&mut Parser<'_>, can_assign: bool);

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
    rules[TokenType::LeftParen as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.grouping()), 
        None, 
        Precedence::None);

    rules[TokenType::Minus as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.unary()), 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Term);

    rules[TokenType::Plus as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Term);

    rules[TokenType::Slash as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Factor);

    rules[TokenType::Star as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Factor);

    rules[TokenType::Number as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.number()), 
        None, 
        Precedence::None);

    rules[TokenType::String as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.string()), 
        None, 
        Precedence::None);

    rules[TokenType::False as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.literal()), 
        None, 
        Precedence::None);

    rules[TokenType::True as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.literal()), 
        None, 
        Precedence::None);

    rules[TokenType::Nil as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.literal()), 
        None, 
        Precedence::None);

    rules[TokenType::Bang as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.unary()), 
        None, 
        Precedence::None);

    rules[TokenType::BangEqual as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Equality);

    rules[TokenType::EqualEqual as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Equality);

    rules[TokenType::Greater as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Comparison);

    rules[TokenType::GreaterEqual as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Comparison);

    rules[TokenType::Less as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Comparison);

    rules[TokenType::LessEqual as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.binary()), 
        Precedence::Comparison);

    rules[TokenType::Identifier as usize] = ParseRule::new(
        Some(|parser, can_assign| parser.variable(can_assign)), 
        None, 
        Precedence::None);

    rules[TokenType::And as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.and(can_assign)), 
        Precedence::And);

    rules[TokenType::Or as usize] = ParseRule::new(
        None, 
        Some(|parser, can_assign| parser.or(can_assign)), 
        Precedence::Or);

    rules
};

impl<'a> Parser<'a> {
    pub fn new(object_manager: &'a mut ObjectManager, intern_strings: &'a mut Table) -> Box<Parser<'a>> {
        Box::new(Parser{
            current: Token{token_type: TokenType::Eof, value: "", line: 0},
            previous: Token{token_type: TokenType::Eof, value: "", line: 0},
            scanner: None,
            has_error: false,
            panic_mode: false,
            chunk: None,
            compiler: Compiler { locals: vec![], scope_depth: 0 },
            object_manager,
            intern_strings,
        })
    }

    pub fn compile(&mut self, source: &'a str, chunk: &'a mut Chunk) -> bool {
        self.scanner = Some(Scanner::new(source));
        self.current = Token{token_type: TokenType::Eof, value: "", line: 0};
        self.previous = Token{token_type: TokenType::Eof, value: "", line: 0};

        self.chunk = Some(chunk);
        self.advance();

        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::Eof, "Expect end of expression.");
        
        self.end_compiler();
        return !self.has_error;
    }

    fn advance(&mut self) {
        self.previous = self.current.clone();
        loop {
            if let Some(scanner) = &mut self.scanner {
                self.current = scanner.scan_token();
                if self.current.token_type != TokenType::Error {
                    break;
                }
    
                self.error_at_current(self.current.value);
            } else {
                panic!("Compiler was not initialized correctly.");
            }
        }
    }

    fn match_token(&mut self, token_type: TokenType) -> bool {
        if !self.check(token_type) {
            return false;
        }

        self.advance();
        true
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.current.token_type == token_type
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
        if let Some(index) = self.current_chunk().find_constant(value) {
            return index as u8;
        }

        let constant_index = self.current_chunk().add_constant(value);
        if constant_index > u8::max_value().into() {
            self.error("Too many constants in one chunk.");
            return 0;
        }
        constant_index as u8
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        self.chunk.as_mut().expect("Chunk is None")
    }

    fn number(&mut self) {
        let value = match self.previous.value.parse::<f64>() {
            Ok(num) => num,
            Err(_) => 0.0,
        };
        self.emit_constant(make_numer_value(value));
    }

    fn string(&mut self) {
        // let content = &self.previous.value[1..self.previous.value.len() - 1];  // The + 1 and - 1 parts trim the leading and trailing quotation marks.
        // let result = self.intern_strings.find(content);
        // match result {
        //     Some(string) => {
        //         self.emit_constant(Value {
        //             value_type: ValueType::ValueObject,
        //             value_as: ValueUnion{object: string as *mut Object},
        //         });
        //     },
        //     None => {
        //         let mut value = make_string_value(&mut self.intern_strings, content);
        //         self.object_manager.push_object_value(&mut value);
        //         self.emit_constant(value);
        //     }
        // }
        let value = make_string_value(
            &mut self.object_manager,
            &mut self.intern_strings,
            &self.previous.value[1..self.previous.value.len() - 1]  // The + 1 and - 1 parts trim the leading and trailing quotation marks.
        );
        self.emit_constant(value);
    }

    fn declaration(&mut self) {
        if self.match_token(TokenType::Var) {
            self.variable_declaration();
        } else {
            self.statement();
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn variable_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");

        if self.match_token(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_byte(OpCode::Nil.to_byte());
        }
        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration.");

        self.define_variable(global);
    }

    fn parse_variable(&mut self, message: &'a str) -> u8 {
        self.consume(TokenType::Identifier, message);

        self.declare_variable();
        if self.compiler.scope_depth > 0 {
            return 0;
        }
        return self.identifier_constant(self.previous.clone());
    }

    fn declare_variable(&mut self) {
        let current_compiler = &self.compiler;
        if current_compiler.scope_depth == 0 { // means top level
            return;
        }

        let mut err = false;
        let variable_name = self.previous.clone();
        for local in current_compiler.locals.iter().rev() {
            if local.depth != -1 && local.depth < current_compiler.scope_depth {
                break;
            }

            if Self::identifier_equal(&variable_name, &local.name) {
                err = true;
                break;
            }
        }
        if err {
            self.error("Already a variable with this name in this scope.");
        }
        
        self.add_local(variable_name);
    }

    fn add_local(&mut self, variable_name: Token<'a>) {
        if self.compiler.locals.len() >= u8::max_value().into() {
            self.error("Too many local variables in function.");
            return;
        }

        // Set 'depth' to -1 in order to mark this variable uninitialized. If the variable
        // declaration expression has an initializer that is parsed correctly, the 'depth'
        // will be set to the scope depth of 'compiler'
        self.compiler.locals.push(Local { name: variable_name, depth: -1 });
    }

    fn identifier_constant(&mut self, previous: Token) -> u8 {
        let value = make_string_value(&mut self.object_manager, &mut self.intern_strings, previous.value);
        self.make_constant(value)
    }

    fn define_variable(&mut self, global: u8) {
        // > 0 means a local variable
        if self.compiler.scope_depth > 0 {
            self.mark_initialized();
            return;
        }
        
        self.emit_bytes(OpCode::DefineGlobal.to_byte(), global);
    }

    fn mark_initialized(&mut self) {
        let current_local_index = self.compiler.locals.len() - 1;
        self.compiler.locals[current_local_index].depth = self.compiler.scope_depth;
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.previous.clone(), can_assign)
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let mut opcode_get: u8 = OpCode::GetLocal.to_byte();
        let mut opcode_set: u8 = OpCode::SetLocal.to_byte();
        let mut index = self.resove_local(&name);
        if index == -1 { // global variable
            index = self.identifier_constant(name) as i32;
            opcode_get = OpCode::GetGlobal.to_byte();
            opcode_set = OpCode::SetGlobal.to_byte();
        }

        if can_assign && self.match_token(TokenType::Equal) {
            self.expression();
            self.emit_bytes(opcode_set, index as u8);
        } else {
            self.emit_bytes(opcode_get, index as u8);
        }
    }

    fn resove_local(&mut self, name: &Token) -> i32 {
        for (index, local) in self.compiler.locals.iter().enumerate().rev() {
            if Self::identifier_equal(&name, &local.name) {
                if local.depth == -1 { // it's not fully defined
                    self.error("Can't read local variable in its own initializer.");
                }
                return index as i32;
            }
        }

        return -1;
    }

    fn identifier_equal(left: &Token, right: &Token) -> bool {
        left.token_type == right.token_type && left.value == right.value
    }

    fn and(&mut self, can_assign: bool) {
        let jump_offset_operand = self.emit_jump_bytes(OpCode::JumpIfFalse.to_byte());
        self.emit_byte(OpCode::Pop.to_byte());
        self.parse_precedence(Precedence::And);
        self.patch_jump_offset(jump_offset_operand);
    }

    fn or(&mut self, can_assign: bool) {
        let jump_offset_operand = self.emit_jump_bytes(OpCode::JumpIfTrue.to_byte());
        self.emit_byte(OpCode::Pop.to_byte());
        self.parse_precedence(Precedence::Or);
        self.patch_jump_offset(jump_offset_operand);
    }

    fn statement(&mut self) {
        if self.match_token(TokenType::If) {
            self.if_statement();
        } else if self.match_token(TokenType::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else if self.match_token(TokenType::While) {
            self.while_statement();
        } else if self.match_token(TokenType::For) {
            self.for_statement();
        } else if self.match_token(TokenType::Print) {
            self.print_statement();
        } else {
            self.expression_statement();
        }
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let jump_offset_operand = self.emit_jump_bytes(OpCode::JumpIfFalse.to_byte());
        self.emit_byte(OpCode::Pop.to_byte()); // to pop the condition result to eliminate the effect on the stack
        self.statement();

        let else_jump_offset_operand = self.emit_jump_bytes(OpCode::Jump.to_byte());
        self.patch_jump_offset(jump_offset_operand);
        self.emit_byte(OpCode::Pop.to_byte()); // This operation is the same as the above 'Pop' operation

        if self.match_token(TokenType::Else) {
            self.statement();
        }
        self.patch_jump_offset(else_jump_offset_operand);
    }

    fn emit_jump_bytes(&mut self, instruction: u8) -> u16 {
        self.emit_byte(instruction);
        // use two bytes for the jump offset operand
        self.emit_byte(0xff);
        self.emit_byte(0xff);
        (self.current_chunk().code.len() - 2) as u16
    }

    fn patch_jump_offset(&mut self, offset: u16) {
        // -2 to adjust for the bytecode for the jump offset itself.
        let jump_offset = self.current_chunk().code.len() as u16 - offset - 2;
        if jump_offset > u16::max_value().into() {
            self.error("Too much code to jump over.");
        }

        self.current_chunk().code[offset as usize] = ((jump_offset >> 8) & 0xff) as u8;
        self.current_chunk().code[offset as usize + 1] = (jump_offset & 0xff) as u8;
    }

    fn begin_scope(&mut self) {
        self.compiler.scope_depth += 1
    }

    fn end_scope(&mut self) {
        self.compiler.scope_depth -= 1;

        loop {
            if self.compiler.locals.is_empty()
                || self.compiler.locals[self.compiler.locals.len() - 1].depth <= self.compiler.scope_depth {
                break;
            }

            self.emit_byte(OpCode::Pop.to_byte());
            self.compiler.locals.pop();
        }
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_byte(OpCode::Print.to_byte());
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk().code.len();

        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let jump_offset_operand = self.emit_jump_bytes(OpCode::JumpIfFalse.to_byte());
        self.emit_byte(OpCode::Pop.to_byte());

        self.statement();
        self.emit_loop(loop_start); // jump to the condition expression of 'while' statement

        self.patch_jump_offset(jump_offset_operand);
        self.emit_byte(OpCode::Pop.to_byte());
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit_byte(OpCode::Loop.to_byte());

        // +2 to skip for the operand of 'Loop' instruction.
        let offset = self.current_chunk().code.len() - loop_start + 2;
        if offset > u16::max_value().into() {
            self.error("Loop body too large.");
        }

        self.emit_byte(((offset as u16) >> 8 & 0xff) as u8);
        self.emit_byte((offset & 0xff) as u8);
    }

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.");
        if self.match_token(TokenType::Semicolon) {
            // no initializer
        } else if self.match_token(TokenType::Var) {
            self.variable_declaration();
        } else {
            self.expression_statement();
        }

        let mut loop_start = self.current_chunk().code.len();
        let mut exit_jump_offset_operand: i32 = -1;
        if !self.match_token(TokenType::Semicolon) { // it has a condition clause
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");

            // Jump out of the loop if the condition is false.
            exit_jump_offset_operand = self.emit_jump_bytes(OpCode::JumpIfFalse.to_byte()) as i32;
            self.emit_byte(OpCode::Pop.to_byte()); // pop the condition result.
        }

        if !self.match_token(TokenType::RightParen) { // it has a increment clause
            let body_jump_offset_operand = self.emit_jump_bytes(OpCode::Jump.to_byte());
            let increment_start = self.current_chunk().code.len();
            self.expression();
            self.emit_byte(OpCode::Pop.to_byte());
            self.consume(TokenType::RightParen, "Expect ')' after for clauses.");

            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump_offset(body_jump_offset_operand);
        }

        self.statement();
        self.emit_loop(loop_start);

        if exit_jump_offset_operand != -1 {
            self.patch_jump_offset(exit_jump_offset_operand as u16);
            self.emit_byte(OpCode::Pop.to_byte()); // pop the condition result.
        }
        self.end_scope();
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after expression.");
        self.emit_byte(OpCode::Pop.to_byte());
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
            TokenType::Bang => self.emit_byte(OpCode::Not.to_byte()),
            TokenType::Minus => self.emit_byte(OpCode::Negate.to_byte()),
            _ => unreachable!("Expect unary operator."),
        }
    }

    fn binary(&mut self) {
        let operator_type = self.previous.token_type;
        let rule = &RULES[operator_type as usize];
        self.parse_precedence((rule.precedence as u8 + 1).into());

        match operator_type {
            TokenType::BangEqual => self.emit_bytes(OpCode::Equal.to_byte(), OpCode::Not.to_byte()),
            TokenType::EqualEqual => self.emit_byte(OpCode::Equal.to_byte()),
            TokenType::Greater => self.emit_byte(OpCode::Greater.to_byte()),
            TokenType::GreaterEqual => self.emit_bytes(OpCode::Less.to_byte(), OpCode::Not.to_byte()),
            TokenType::Less => self.emit_byte(OpCode::Less.to_byte()),
            TokenType::LessEqual => self.emit_bytes(OpCode::Greater.to_byte(), OpCode::Not.to_byte()),
            TokenType::Plus => self.emit_byte(OpCode::Add.to_byte()),
            TokenType::Minus => self.emit_byte(OpCode::Subtract.to_byte()),
            TokenType::Star => self.emit_byte(OpCode::Multiply.to_byte()),
            TokenType::Slash => self.emit_byte(OpCode::Divide.to_byte()),
            _ => unreachable!("Unexpected binary operator: {}", operator_type)
        }
    }

    fn literal(&mut self) {
        let operator_type = self.previous.token_type;
        match operator_type {
            TokenType::False => self.emit_byte(OpCode::False.to_byte()),
            TokenType::True => self.emit_byte(OpCode::True.to_byte()),
            TokenType::Nil => self.emit_byte(OpCode::Nil.to_byte()),
            _ => unreachable!("Unexpected literal operator: {}", operator_type)
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        let can_assign = precedence <= Precedence::Assignment;
        if let Some(prefix) = &RULES[self.previous.token_type as usize].prefix {
            prefix(self, can_assign);
        } else {
            self.error("Expect expression.");
            return;
        }

        while precedence as u8 <= (&RULES[self.current.token_type as usize]).precedence as u8 {
            self.advance();
            if let Some(infix) = &RULES[self.previous.token_type as usize].infix {
                infix(self, can_assign);
            } else {
                self.error("Expect infix parse function.");
                return;
            }
        }

        if can_assign && self.match_token(TokenType::Equal) {
            self.error("Invalid assignment target.");
        }
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;
        while self.current.token_type != TokenType::Eof {
            if self.previous.token_type == TokenType::Semicolon {
                return;
            }

            match self.current.token_type {
                token_type if matches!(token_type,
                    TokenType::Class |
                    TokenType::Fun |
                    TokenType::Var |
                    TokenType::For |
                    TokenType::If |
                    TokenType::While |
                    TokenType::Print |
                    TokenType::Return) => return,
                _ => ()
            }

            self.advance()
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

#[cfg(test)]
mod tests {
    use crate::chunk::Chunk;

    use super::*;

    impl<'a> Parser<'a> {
        pub fn chunk(&mut self) -> &mut Chunk {
            self.chunk.as_mut().expect("Chunk is None")
        }
    }

    #[test]
    fn test_compile() {
        let mut chunk = Chunk::new();
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut *object_manager, &mut *intern_strings);
        let result = parser.compile("!(5 - 4 > 3 * 2 == !nil);", &mut *chunk);
        assert!(result);

        let chunk = parser.chunk();

        // 00000000 00000001 Constant            0 '5'
        // 00000002        | Constant            1 '4'
        // 00000004        | Subtract
        // 00000005        | Constant            2 '3'
        // 00000007        | Constant            3 '2'
        // 00000009        | Multiply
        // 00000010        | Greater
        // 00000011        | Nil
        // 00000012        | Not
        // 00000013        | Equal
        // 00000014        | Not
        // 00000015        | Pop
        // 00000016        | Return
        assert!(chunk.constants[0] == Value {
            value_type: ValueType::ValueNumber,
            value_as: ValueUnion{number: 5.0}});

        assert!(chunk.constants[1] == Value {
            value_type: ValueType::ValueNumber,
            value_as: ValueUnion{number: 4.0}});

        assert!(chunk.code[0] == OpCode::Constant.to_byte());
        assert!(chunk.code[1] == 0); // constant index
        assert!(chunk.code[2] == OpCode::Constant.to_byte());
        assert!(chunk.code[3] == 1); // constant index
        assert!(chunk.code[4] == OpCode::Subtract.to_byte());
        assert!(chunk.code[5] == OpCode::Constant.to_byte());
        assert!(chunk.code[6] == 2); // constant index
        assert!(chunk.code[7] == OpCode::Constant.to_byte());
        assert!(chunk.code[8] == 3); // constant index
        assert!(chunk.code[9] == OpCode::Multiply.to_byte());
        assert!(chunk.code[10] == OpCode::Greater.to_byte());
        assert!(chunk.code[11] == OpCode::Nil.to_byte());
        assert!(chunk.code[12] == OpCode::Not.to_byte());
        assert!(chunk.code[13] == OpCode::Equal.to_byte());
        assert!(chunk.code[14] == OpCode::Not.to_byte());
        assert!(chunk.code[15] == OpCode::Pop.to_byte());
        assert!(chunk.code[16] == OpCode::Return.to_byte());
    }

    #[test]
    fn test_intern_strings() {
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut *object_manager, &mut *intern_strings);
        
        let mut chunk1 = Chunk::new();
        let result = parser.compile("\"this is a test string\";", &mut *chunk1);
        assert!(result);

        let mut chunk2 = Chunk::new();
        let result = parser.compile("\"this is a test string\";", &mut *chunk2);
        assert!(result);

        assert!(intern_strings.len() == 1);
    }
}