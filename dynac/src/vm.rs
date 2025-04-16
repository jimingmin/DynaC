use crate::{chunk::{self, OpCode}, debug, value::{as_bool, as_number, is_bool, is_nil, is_number, make_bool_value, make_nil_value, make_numer_value, print_value, Value, ValueType, ValueUnion}};

const MAX_STACK_SIZE: usize = 256;

pub struct VM {
    pub chunk: Box<chunk::Chunk>,
    pub ip: usize,
    pub stack: [Value; MAX_STACK_SIZE],
    pub stack_top_pos: usize,
}

pub enum InterpretResult {
    InterpretOk,
    InterpretCompileError,
    InterpretRuntimeError,
}

impl VM {
    pub fn new() -> Box<VM> {
        let chunk = chunk::Chunk::new();
        Box::new(VM{chunk, ip: 0, stack:[Value {
            value_type: ValueType::ValueNil,
            value_as: ValueUnion{number: 0.0},
        }; MAX_STACK_SIZE], stack_top_pos: 0})
    }

    pub fn interpret(&mut self, source: &str) -> InterpretResult {
        self.compile(source)
        // if self.compile(source) {
        //     return InterpretResult::InterpretCompileError;
        // }
        
        // InterpretResult::InterpretOk
    }

    // pub fn interpret(&mut self, chunk: Box<chunk::Chunk>) -> InterpretResult {
    //     self.chunk = chunk;

    //     match self.run() {
    //         Ok(result) => result,
    //         Err(e) => {
    //             println!("Error during interpretation: {}", e);
    //             return InterpretResult::InterpretRuntimeError;
    //         },
    //     }
    // }

    fn compile(&mut self, source: &str) -> InterpretResult {
        match self.run() {
            Ok(result) => result,
            Err(e) => {
                println!("Error during interpretation: {}", e);
                return InterpretResult::InterpretRuntimeError;
            },
        }
    }

    fn push(&mut self, value: Value) {
        if self.stack_top_pos < MAX_STACK_SIZE {
            self.stack[self.stack_top_pos] = value;
            self.stack_top_pos += 1;
        } else {
            panic!("Stack overflow");
        }
    }

    fn pop(&mut self) -> Value {
        if self.stack_top_pos > 0 {
            self.stack_top_pos -= 1;
            self.stack[self.stack_top_pos]
        } else {
            panic!("Stack underflow");
        }
    }

    fn peek(&self) -> Option<Value> {
        if self.stack_top_pos > 0 {
            Some(self.stack[self.stack_top_pos - 1])
        } else {
            None
        }
    }

    fn peek_steps(&self, distance: usize) -> Option<Value> {
        if self.stack_top_pos > 0 {
            Some(self.stack[self.stack_top_pos - distance - 1])
        } else {
            None
        }
    }

    fn is_falsey(value: &Value) -> bool {
        is_nil(value) || (is_bool(value) && !as_bool(value))
    }

    fn run(&mut self) -> Result<InterpretResult, &'static str> {
        loop {
            debug_feature::disassemble_instruction(&self);

            let instruction = match self.read_byte() {
                Some(byte) => chunk::OpCode::from_byte(byte),
                None => return Err("Unexpected end of bytecode"),
            };

            match instruction {
                Some(chunk::OpCode::Constant) => {
                    if let Some(constant) = self.read_constant() {
                        self.push(constant);
                    }
                }
                Some(chunk::OpCode::Nil) => {
                    self.push(make_nil_value());
                }
                Some(chunk::OpCode::True) => {
                    self.push(make_bool_value(true));
                }
                Some(chunk::OpCode::False) => {
                    self.push(make_bool_value(false));
                }
                Some(chunk::OpCode::Equal) => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(make_bool_value(a == b));
                }
                Some(chunk::OpCode::Greater) => {
                    let result = self.binary_op(chunk::OpCode::Greater);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Less) => {
                    let result = self.binary_op(chunk::OpCode::Less);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Add) => {
                    let result = self.binary_op(chunk::OpCode::Add);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Subtract) => {
                    let result = self.binary_op(chunk::OpCode::Subtract);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Multiply) => {
                    let result = self.binary_op(chunk::OpCode::Multiply);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Divide) => {
                    let result = self.binary_op(chunk::OpCode::Divide);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Not) => {
                    let byte = self.pop();
                    self.push(make_bool_value(Self::is_falsey(&byte)));
                }
                Some(chunk::OpCode::Negate) => {
                    if let Some(value) = self.peek_steps(0) {
                        if !is_number(&value) {
                            return Err("Operand must be a number");
                        }
                    }
                    let byte = self.pop();
                    let value = make_numer_value(-as_number(&byte));
                    self.push(value);
                }
                Some(chunk::OpCode::Return) => {
                    print_value(self.pop());
                    println!();
                    return Ok(InterpretResult::InterpretOk);
                }
                _ => return Err("Unknown opcode"),
            }
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        if self.ip < self.chunk.code.len() {
            let current_byte = self.chunk.code[self.ip];
            self.ip += 1;
            Some(current_byte)
        } else {
            None
        }
    }

    fn read_constant(&mut self) -> Option<Value> {
        let instruction = match self.read_byte() {
            Some(byte) => byte,
            None => return None,
        };
        Some(self.chunk.constants[instruction as usize])
    }

    // fn BinaryOperation(&mut self, op_code: chunk::OpCode) {
    //     let a = self.pop();
    //     let b = self.pop();
    //     match op_code {
    //         chunk::OpCode::Add => self.push(a + b),
    //         chunk::OpCode::Subtract => self.push(a - b),
    //         chunk::OpCode::Multiply => self.push(a * b),
    //         chunk::OpCode::Divide => self.push(a / b),
    //         _ => panic!("Unknown binary operator"),
    //     }
    // }

    fn binary_op(
        &mut self,
        op_code: chunk::OpCode,
    ) -> Result<InterpretResult, &'static str> {
            if self.stack_top_pos < 2 {
                return Err("");
            }

            if let Some(b) = self.peek_steps(0) {
                if !is_number(&b) {
                    return Err("Second operand must be a number");
                }
            }

            if let Some(a) = self.peek_steps(1) {
                if !is_number(&a) {
                    return Err("First operand must be a number");
                }
            }
            let value_b = as_number(&self.pop());
            let value_a = as_number(&self.pop());
            match op_code {
                chunk::OpCode::Greater => {
                    self.push(make_bool_value(value_a > value_b))
                }
                chunk::OpCode::Less => {
                    self.push(make_bool_value(value_a < value_b))
                }
                chunk::OpCode::Add => {
                    self.push(make_numer_value(value_a + value_b))
                }
                chunk::OpCode::Subtract => {
                    self.push(make_numer_value(value_a - value_b))
                }
                chunk::OpCode::Multiply => {
                    self.push(make_numer_value(value_a * value_b))
                }
                chunk::OpCode::Divide => {
                    self.push(make_numer_value(value_a / value_b))
                }
                _ => return Err("Unknown binary operator"),
            };

            Ok(InterpretResult::InterpretOk)
        }
}

#[cfg(feature = "debug_trace_execution")]
mod debug_feature {
    use super::*;

    pub fn disassemble_instruction(vm: &VM) {
        print!("{: >17}", "");
        for slot in &vm.stack[0..vm.stack_top_pos] {
            print!(" [ ");
            print_value(*slot);
            print!(" ]");
        }
        println!();
        debug::disassemble_instruction(&vm.chunk, vm.ip);
    }
}

#[cfg(not(feature = "debug_trace_execution"))]
mod debug_feature {
    use super::*;

    pub fn disassemble_instruction(vm: &VM) {}
}
