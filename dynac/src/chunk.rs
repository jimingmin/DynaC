use strum_macros::{EnumString, Display};
use crate::value::{Value, ValueArray};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
pub enum OpCode {
    OpConstant,
    OpAdd,
    OpSubtract,
    OpMultiply,
    OpDivide,
    OpNegate,
    OpReturn,
    //Unknown(u8),
}

const OPCODE_ARRAY: [Option<OpCode>; 256] = {
    let mut arr = [None; 256];

    arr[OpCode::OpConstant as u8 as usize] = Some(OpCode::OpConstant);
    arr[OpCode::OpAdd as u8 as usize] = Some(OpCode::OpAdd);
    arr[OpCode::OpSubtract as u8 as usize] = Some(OpCode::OpSubtract);
    arr[OpCode::OpMultiply as u8 as usize] = Some(OpCode::OpMultiply);
    arr[OpCode::OpDivide as u8 as usize] = Some(OpCode::OpDivide);
    arr[OpCode::OpNegate as u8 as usize] = Some(OpCode::OpNegate);
    arr[OpCode::OpReturn as u8 as usize] = Some(OpCode::OpReturn);
    arr
};

#[allow(non_snake_case)]
impl OpCode {
    #[inline(always)]
    pub fn from_byte(byte: u8) -> Option<Self> {
        OPCODE_ARRAY[byte as usize]
    }

    #[inline(always)]
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    pub fn byte_to_string(byte: &Option<OpCode>) -> String {
        match byte {
            Some(code) => code.to_string(),
            None => "None".to_string(),
        }
    }
}

pub struct Chunk {
    pub code: Vec<u8>,
    pub lines: Vec<u32>,
    pub constants: ValueArray,
}

impl Chunk {
    pub fn new() -> Box<Chunk> {
        Box::new(Chunk{code:vec![], constants:vec![], lines:vec![]})
    }

    pub fn write(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.lines.push(line)
    }

    pub fn add_constants(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

