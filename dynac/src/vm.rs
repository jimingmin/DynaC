use crate::{chunk::{self, OpCode}, debug, value::{print_value, Value}};

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
        Box::new(VM{chunk, ip: 0, stack:[0.0; MAX_STACK_SIZE], stack_top_pos: 0})
    }

    pub fn interpret(&mut self, source: &str) -> InterpretResult {
        if self.compile(source) {
            return InterpretResult::InterpretCompileError;
        }
        
        InterpretResult::InterpretOk
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

    fn compile(&mut self, source: &str) -> bool {
        true
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
                Some(chunk::OpCode::Add) => {
                    self.BinaryOperation(chunk::OpCode::Add)
                }
                Some(chunk::OpCode::Subtract) => {
                    self.BinaryOperation(chunk::OpCode::Subtract)
                }
                Some(chunk::OpCode::Multiply) => {
                    self.BinaryOperation(chunk::OpCode::Multiply)
                }
                Some(chunk::OpCode::Divide) => {
                    self.BinaryOperation(chunk::OpCode::Divide)
                }
                Some(chunk::OpCode::Negate) => {
                    let byte = -self.pop();
                    self.push(byte);
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

    fn BinaryOperation(&mut self, op_code: chunk::OpCode) {
        let a = self.pop();
        let b = self.pop();
        match op_code {
            chunk::OpCode::Add => self.push(a + b),
            chunk::OpCode::Subtract => self.push(a - b),
            chunk::OpCode::Multiply => self.push(a * b),
            chunk::OpCode::Divide => self.push(a / b),
            _ => panic!("Unknown binary operator"),
        }
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
