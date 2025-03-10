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

    pub fn interpret(&mut self, chunk: Box<chunk::Chunk>) -> InterpretResult {
        self.chunk = chunk;

        match self.run() {
            Ok(result) => result,
            Err(e) => {
                println!("Error during interpretation: {}", e);
                return InterpretResult::InterpretRuntimeError;
            },
        }
    }

    pub fn push(&mut self, value: Value) {
        if self.stack_top_pos < MAX_STACK_SIZE {
            self.stack[self.stack_top_pos] = value;
            self.stack_top_pos += 1;
        } else {
            panic!("Stack overflow");
        }
    }

    pub fn pop(&mut self) -> Value {
        if self.stack_top_pos > 0 {
            self.stack_top_pos -= 1;
            self.stack[self.stack_top_pos]
        } else {
            panic!("Stack underflow");
        }
    }

    pub fn peek(&self) -> Option<Value> {
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
                Some(byte) => byte,
                None => return Err("Unexpected end of bytecode"),
            };

            match instruction {
                chunk::OpCode::OP_CONSTANT_CODE => {
                    if let Some(constant) = self.read_constant() {
                        self.push(constant);
                    }
                }
                chunk::OpCode::OP_NEGATE_CODE => {
                    let byte = -self.pop();
                    self.push(byte);
                }
                chunk::OpCode::OP_RETURN_CODE => {
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
