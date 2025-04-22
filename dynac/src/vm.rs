use crate::{chunk::{self, Chunk, OpCode}, compiler::{self, Parser}, debug, object::ObjectString, value::{as_bool, as_number, as_object, is_bool, is_nil, is_number, is_string, make_bool_value, make_nil_value, make_numer_value, make_string_value, print_value, Value, ValueType, ValueUnion}};

const MAX_STACK_SIZE: usize = 256;

pub struct VM {
    pub chunk: Box<chunk::Chunk>,
    pub ip: usize,
    pub stack: [Value; MAX_STACK_SIZE],
    pub stack_top_pos: usize,
}

#[derive(PartialEq)]
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
    }

    fn compile(&mut self, source: &str) -> InterpretResult {
        let mut parser = Parser::new();
        parser.compile(source, &mut *self.chunk);

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
                    if let Some(value_b) = self.peek_steps(0) {
                        if let Some(value_a) = self.peek_steps(1) {
                            if is_string(&value_a) && is_string(&value_b) {
                                unsafe {
                                    let string_b = &*(as_object(&self.pop()) as *const ObjectString);
                                    let string_a = &*(as_object(&self.pop()) as *const ObjectString);
                                    let mut combination = String::with_capacity(string_a.content.len() + string_b.content.len());
                                    combination.push_str(string_a.content.as_str());
                                    combination.push_str(string_b.content.as_str());
                                    let combinated_value = make_string_value(combination.as_str());
                                    self.push(combinated_value);
                                }
                            } else if is_number(&value_a) && is_number(&value_b) {
                                let number_a = as_number(&value_a);
                                let number_b = as_number(&value_b);
                                self.push(make_numer_value(number_a + number_b));
                            } else {
                                return Err("Operands must be two numbers or two strings.");
                            }
                        } else {
                            return Err("There is a lack of second operand in the Add Operation.");
                        }
                    } else {
                        return Err("There is a lack of operands in the Add Operation.");
                    }

                    // let result = self.binary_op(chunk::OpCode::Add);
                    // match result {
                    //     Err(_) => return result,
                    //     _ => (),
                    // }
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
                    print_value(&self.pop());
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
        if vm.stack_top_pos < 1 {
            return;
        }

        print!("{: >17}", "");
        for slot in &vm.stack[0..vm.stack_top_pos] {
            print!(" [ ");
            print_value(slot);
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


#[cfg(test)]
mod tests {
    use crate::vm::InterpretResult;

    use super::VM;

    #[test]
    fn test_comparison_expression() {
        let mut vm = VM::new();
        assert!(vm.interpret("!(5 - 4 > 3 * 2 == !nil)") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_string_concatenate() {
        let mut vm = VM::new();
        assert!(vm.interpret("\"st\" + \"ri\" + \"ng\"") == InterpretResult::InterpretOk);
    }
}