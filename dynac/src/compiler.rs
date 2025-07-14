use crate::{chunk::{self, Chunk, OpCode}, compiler, objects::{object::Object, object_function::ObjectFunction, object_manager::{self, ObjectManager}}, scanner::{self, Scanner, Token, TokenType}, table::Table, value::{self, *}};
use std::{any::Any, cell::{Ref, RefCell, RefMut}, error, f64, io::Write, mem, rc::Rc, thread::current};

pub struct Parser<'a> {
    current: Token<'a>,
    previous: Token<'a>,
    scanner: Option<Box<Scanner<'a>>>,
    has_error: bool,
    panic_mode: bool,
    compilers: Vec<Compiler<'a>>,
    object_manager: &'a mut ObjectManager,
    intern_strings: &'a mut Table,
}

struct Local<'a> {
    name: Token<'a>,
    depth: i32,
}

#[derive(Clone)]
struct Upvalue {
    index: usize,
    is_local: bool,
}

#[derive(PartialEq)]
enum FunctionType {
    Function,
    Script,
}

struct Compiler<'a> {
    function: Box<ObjectFunction>,
    function_type: FunctionType,
    locals: Vec<Local<'a>>,
    upvalues: Vec<Upvalue>,
    scope_depth: i32,
}

impl<'a> Compiler<'a> {
    pub fn new(function_type: FunctionType) -> Self {
        Compiler {
            function: Box::new(ObjectFunction::new(0, String::new())),
            function_type,
            locals: vec![],
            upvalues: vec![],
            scope_depth: 0
        }
    }    
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
        Some(|parser, can_assign| parser.call(can_assign)),
        Precedence::Call);

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
        let mut parser = Box::new(Parser{
            current: Token{token_type: TokenType::Eof, value: "", line: 0},
            previous: Token{token_type: TokenType::Eof, value: "", line: 0},
            scanner: None,
            has_error: false,
            panic_mode: false,
            compilers: vec![],
            object_manager,
            intern_strings,
        });
        parser.init_compiler(FunctionType::Script);
        parser
    }

    pub fn compile(&mut self, source: &'a str) -> Option<Box<ObjectFunction>> {
        self.scanner = Some(Scanner::new(source));
        self.current = Token{token_type: TokenType::Eof, value: "", line: 0};
        self.previous = Token{token_type: TokenType::Eof, value: "", line: 0};

        self.advance();

        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::Eof, "Expect end of expression.");
        
        return self.end_compiler();
    }

    fn specific_compiler(&self, compiler_index: usize) -> &Compiler<'a> {
        self.compilers.get(compiler_index).expect("compiler index is invalid.")
    }

    fn specific_compiler_mut(&mut self, compiler_index: usize) -> &mut Compiler<'a> {
        self.compilers.get_mut(compiler_index).expect("compiler index is invalid.")
    }

    fn current_compiler(&self) -> &Compiler<'a> {
        self.compilers.last().expect("No compiler.")
    }

    fn current_compiler_mut(&mut self) -> &mut Compiler<'a> {
        self.compilers.last_mut().expect("No compiler.")
    }

    fn current_function(&self) -> &Box<ObjectFunction> {
        &self.current_compiler().function
    }

    fn current_function_mut(&mut self) -> &mut Box<ObjectFunction> {
        &mut self.current_compiler_mut().function
    }

    fn current_chunk(&self) -> &Box<Chunk> {
        &self.current_function().chunk
    }

    fn current_chunk_mut(&mut self) -> &mut Box<Chunk> {
        &mut self.current_function_mut().chunk
        //&mut (*self.compiler.function.get_mut().chunk.as_mut())
        //self.chunk.as_mut().expect("Chunk is None")
    }

    fn current_locals(&self) -> &Vec<Local<'a>> {
        &self.current_compiler().locals
    }

    fn current_locals_mut(&mut self) -> &mut Vec<Local<'a>> {
        &mut self.current_compiler_mut().locals
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
        self.current_chunk_mut().write(byte, line);
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
        self.emit_byte(chunk::OpCode::Nil.to_byte());
        self.emit_byte(chunk::OpCode::Return.to_byte());
    }

    fn init_compiler(&mut self, function_type: FunctionType) {
        let mut compiler = Compiler::new(function_type);
        // When compiling a function declaration, we call init_compiler() right after
        // we parse the function’s name. That means we can grab the name right then
        // from the previous token.
        if compiler.function_type != FunctionType::Script {
            compiler.function.name = self.previous.value.to_string();
        }

        // the compiler sets aside stack slot zero that stores the function being called
        compiler.locals.push(Local {
            name: Token {
                token_type: TokenType::Eof,
                value: "",
                line: 0,
            }, 
            depth: 0 });
        self.compilers.push(compiler);
    }

    fn end_compiler(&mut self) -> Option<Box<ObjectFunction>> {
        self.emit_return();

        if self.current_function().name.is_empty() {
            debug_feature::disassemble_chunk(self, "<script>");
        } else {
            let function_name = &self.current_function().name.clone();
            debug_feature::disassemble_chunk(self, function_name);
        }
        
        let function = mem::replace(&mut self.current_compiler_mut().function, 
        Box::new(ObjectFunction::new(0, "".to_string())));
        self.compilers.pop();
        Some(function)
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        if let Some(index) = self.current_chunk().find_constant(value) {
            return index as u8;
        }

        let constant_index = self.current_chunk_mut().add_constant(value);
        if constant_index > u8::max_value().into() {
            self.error("Too many constants in one chunk.");
            return 0;
        }
        constant_index as u8
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
        } else if self.match_token(TokenType::Fn) {
            self.function_declaration();
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
        if self.current_compiler().scope_depth > 0 {
            return 0;
        }
        return self.identifier_constant(self.previous.clone());
    }

    fn declare_variable(&mut self) {
        if self.current_compiler().scope_depth == 0 { // means top level
            return;
        }

        let mut err = false;
        let variable_name = self.previous.clone();
        let scope_depth = self.current_compiler().scope_depth;
        let current_locals = self.current_locals();
        for local in current_locals.iter().rev() {
            if local.depth != -1 && local.depth < scope_depth {
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
        if self.current_locals().len() >= u8::max_value().into() {
            self.error("Too many local variables in function.");
            return;
        }

        // Set 'depth' to -1 in order to mark this variable uninitialized. If the variable
        // declaration expression has an initializer that is parsed correctly, the 'depth'
        // will be set to the scope depth of 'compiler'
        self.current_locals_mut().push(Local { name: variable_name, depth: -1 });
    }

    // fn compiler_ptr(&mut self) -> *mut Compiler<'a> {
    //     &mut self.current_compiler() as *mut Compiler<'a>
    // }

    fn function_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.");
        self.mark_initialized();
        self.function(FunctionType::Function);
        self.define_variable(global);
    }

    fn function(&mut self, function_type: FunctionType) {
        self.init_compiler(function_type);

        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        if !self.check(TokenType::RightParen) {
            loop {
                self.current_function_mut().arity += 1;
                if self.current_function_mut().arity >= 255 {
                    self.error("Can't have more than 255 parameters.");
                }
                let constant = self.parse_variable("Expect parameter name.");
                self.define_variable(constant);

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.");
        self.block();

        let upvalues = self.current_compiler().upvalues.clone();

        let mut object_function = self.end_compiler().expect("Unexpected function object.");
        object_function.upvalue_count = upvalues.len();
        let object_function_ptr = Box::into_raw(object_function);
        let function_constant_index = self.make_constant(make_function_value(object_function_ptr));
        //self.emit_bytes(OpCode::Constant.to_byte(), function_constant_index);
        self.emit_bytes(OpCode::Closure.to_byte(), function_constant_index);

        for upvalue in upvalues.iter() {
            self.emit_byte(if upvalue.is_local { 1 } else { 0 });
            self.emit_byte(upvalue.index as u8);
        }
    }

    fn argument_list(&mut self) -> u8 {
        let mut argument_count = 0;
        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();
                if argument_count >= 255 {
                    self.error("Can't have more than 255 arguments.");
                }
                argument_count += 1;
                
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        argument_count
    }

    fn identifier_constant(&mut self, previous: Token) -> u8 {
        let value = make_string_value(&mut self.object_manager, &mut self.intern_strings, previous.value);
        self.make_constant(value)
    }

    fn define_variable(&mut self, global: u8) {
        // > 0 means a local variable
        if self.current_compiler().scope_depth > 0 {
            self.mark_initialized();
            return;
        }
        
        self.emit_bytes(OpCode::DefineGlobal.to_byte(), global);
    }

    fn mark_initialized(&mut self) {
        let scope_depth = self.current_compiler().scope_depth;
        if scope_depth == 0 {
            return;
        }

        // let current_local_index = self.current_locals().len() - 1;
        // self.current_locals_mut()[current_local_index].depth = scope_depth;
        self.current_locals_mut().last_mut().unwrap().depth = scope_depth;
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.previous.clone(), can_assign)
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let mut opcode_get: u8 = OpCode::GetLocal.to_byte();
        let mut opcode_set: u8 = OpCode::SetLocal.to_byte();
        let current_compiler_index = self.compilers.len() - 1;
        let mut index = self.resolve_local(current_compiler_index, &name);
        if index == -1 {
            index = self.resolve_upvalue(current_compiler_index, &name);
            if index == -1 { // global variable
                index = self.identifier_constant(name) as i32;
                opcode_get = OpCode::GetGlobal.to_byte();
                opcode_set = OpCode::SetGlobal.to_byte();
            } else { // upvalue
                opcode_get = OpCode::GetUpvalue.to_byte();
                opcode_set = OpCode::SetUpvalue.to_byte();
            }
        }

        if can_assign && self.match_token(TokenType::Equal) {
            self.expression();
            self.emit_bytes(opcode_set, index as u8);
        } else {
            self.emit_bytes(opcode_get, index as u8);
        }
    }

    fn resolve_local(&mut self, compiler_index: usize, name: &Token) -> i32 {
        let compiler = self.specific_compiler(compiler_index);
        let locals = &compiler.locals;
        for (index, local) in locals.iter().enumerate().rev() {
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

    fn resolve_upvalue(&mut self, compiler_index: usize, name: &Token) -> i32 {
        if compiler_index == 0 {
            return -1;
        }
        let local = self.resolve_local(compiler_index - 1, name);
        if local != -1 {
            return self.add_upvalue(compiler_index, local, true) as i32;
        }

        let upvalue = self.resolve_upvalue(compiler_index - 1, name);
        if upvalue != -1 {
            return self.add_upvalue(compiler_index, upvalue, false) as i32;
        }

        return -1;
    }

    fn add_upvalue(&mut self, compiler_index: usize, local: i32, is_local: bool) -> usize {
        let mut compiler = self.specific_compiler_mut(compiler_index);
        for (index, upvalue) in compiler.upvalues.iter().enumerate() {
            if upvalue.is_local == is_local && upvalue.index == local as usize {
                return index;
            }
        }
        compiler.upvalues.push(Upvalue { index: local as usize, is_local });
        compiler.function.upvalue_count = compiler.upvalues.len();
        return compiler.function.upvalue_count - 1;
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
        } else if self.match_token(TokenType::Return) {
            self.return_statement();
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
        (self.current_chunk().len() - 2) as u16
    }

    fn patch_jump_offset(&mut self, offset: u16) {
        // -2 to adjust for the bytecode for the jump offset itself.
        let jump_offset = self.current_chunk().len() as u16 - offset - 2;
        if jump_offset > u16::max_value().into() {
            self.error("Too much code to jump over.");
        }

        let mut current_chunk = self.current_chunk_mut();
        current_chunk.write_by_offset(offset as usize, ((jump_offset >> 8) & 0xff) as u8);
        current_chunk.write_by_offset(offset as usize + 1, (jump_offset & 0xff) as u8);
    }

    fn begin_scope(&mut self) {
        self.current_compiler_mut().scope_depth += 1
    }

    fn end_scope(&mut self) {
        self.current_compiler_mut().scope_depth -= 1;
        let scope_depth = self.current_compiler().scope_depth;
        loop {
            
            let current_locals = self.current_locals();
            if current_locals.is_empty() || current_locals.last().unwrap().depth <= scope_depth {
                break;
            }

            self.emit_byte(OpCode::Pop.to_byte());
            self.current_locals_mut().pop();
        }
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn return_statement(&mut self) {
        if self.current_compiler().function_type == FunctionType::Script {
            self.error("Can't return from top-level code.");
        }

        if self.match_token(TokenType::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after return value.");
            self.emit_byte(OpCode::Return.to_byte());
        }
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_byte(OpCode::Print.to_byte());
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk().len();

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
        let offset = self.current_chunk().len() - loop_start + 2;
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

        let mut loop_start = self.current_chunk().len();
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
            let increment_start = self.current_chunk().len();
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

    fn call(&mut self, can_assign: bool) {
        let argument_count = self.argument_list();
        self.emit_bytes(OpCode::Call.to_byte(), argument_count);
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
                    TokenType::Fn |
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

    pub fn disassemble_chunk(parser: &mut Parser, name: &str) {
        if !parser.has_error {
            debug::disassemble_chunk(&parser.current_chunk(), name);
        }
    }
}

#[cfg(not(feature = "debug_print_code"))]
mod debug_feature {
    use super::*;

    pub fn disassemble_chunk(parser: &Parser, name: &str) {}
}

#[cfg(test)]
mod tests {
    use crate::chunk::Chunk;

    use super::*;

    // impl<'a> Parser<'a> {
    //     pub fn chunk(&mut self) -> &mut Chunk {
    //         self.chunk.as_mut().expect("Chunk is None")
    //     }
    // }

    #[test]
    fn test_compile() {
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut object_manager, &mut intern_strings);
        let result = parser.compile("!(5 - 4 > 3 * 2 == !nil);");
        assert!(result.is_some());
        
        let function = &result.unwrap();
        let chunk = &function.chunk;

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
        assert!(*chunk.get_constant(0) == Value {
            value_type: ValueType::ValueNumber,
            value_as: ValueUnion{number: 5.0}});

        assert!(*chunk.get_constant(1) == Value {
            value_type: ValueType::ValueNumber,
            value_as: ValueUnion{number: 4.0}});

        assert!(chunk.read_from_offset(0).unwrap() == OpCode::Constant.to_byte());
        assert!(chunk.read_from_offset(1).unwrap() == 0); // constant index
        assert!(chunk.read_from_offset(2).unwrap() == OpCode::Constant.to_byte());
        assert!(chunk.read_from_offset(3).unwrap() == 1); // constant index
        assert!(chunk.read_from_offset(4).unwrap() == OpCode::Subtract.to_byte());
        assert!(chunk.read_from_offset(5).unwrap() == OpCode::Constant.to_byte());
        assert!(chunk.read_from_offset(6).unwrap() == 2); // constant index
        assert!(chunk.read_from_offset(7).unwrap() == OpCode::Constant.to_byte());
        assert!(chunk.read_from_offset(8).unwrap() == 3); // constant index
        assert!(chunk.read_from_offset(9).unwrap() == OpCode::Multiply.to_byte());
        assert!(chunk.read_from_offset(10).unwrap() == OpCode::Greater.to_byte());
        assert!(chunk.read_from_offset(11).unwrap() == OpCode::Nil.to_byte());
        assert!(chunk.read_from_offset(12).unwrap() == OpCode::Not.to_byte());
        assert!(chunk.read_from_offset(13).unwrap() == OpCode::Equal.to_byte());
        assert!(chunk.read_from_offset(14).unwrap() == OpCode::Not.to_byte());
        assert!(chunk.read_from_offset(15).unwrap() == OpCode::Pop.to_byte());
        assert!(chunk.read_from_offset(16).unwrap() == OpCode::Nil.to_byte());
        assert!(chunk.read_from_offset(17).unwrap() == OpCode::Return.to_byte());
    }

    #[test]
    fn test_intern_strings() {
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut object_manager, &mut intern_strings);
        
        let result = parser.compile("\"this is a test string\";");
        assert!(result.is_some());

        parser = Parser::new(&mut object_manager, &mut intern_strings);
        let result = parser.compile("\"this is a test string\";");
        assert!(result.is_some());

        assert!(intern_strings.len() == 1);
    }

    #[test]
    fn test_function_declaration() {
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut object_manager, &mut intern_strings);
        
        let result = parser.compile(
            "fn areWeHavingItYet() {
                        print \"Yes we are!\";
                    }
                    print areWeHavingItYet;");
        assert!(result.is_some());
    }

    #[test]
    fn test_function_with_arguments() {
        let mut object_manager = ObjectManager::new();
        let mut intern_strings = Table::new();
        let mut parser = Parser::new(&mut object_manager, &mut intern_strings);
        
        let result = parser.compile(
            "fn sum(a, b, c) {
                        return a + b + c;
                    }
                    print 4 + sum(5, 6, 7);");
        assert!(result.is_some());
    }
}