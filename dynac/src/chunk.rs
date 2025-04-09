use strum_macros::{EnumString, Display};
use crate::value::{Value, ValueArray};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
pub enum OpCode {
    Constant,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Return,
    //Unknown(u8),
}

const OPCODE_ARRAY: [Option<OpCode>; 256] = {
    let mut arr = [None; 256];

    arr[OpCode::Constant as u8 as usize] = Some(OpCode::Constant);
    arr[OpCode::Add as u8 as usize] = Some(OpCode::Add);
    arr[OpCode::Subtract as u8 as usize] = Some(OpCode::Subtract);
    arr[OpCode::Multiply as u8 as usize] = Some(OpCode::Multiply);
    arr[OpCode::Divide as u8 as usize] = Some(OpCode::Divide);
    arr[OpCode::Negate as u8 as usize] = Some(OpCode::Negate);
    arr[OpCode::Return as u8 as usize] = Some(OpCode::Return);
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
    pub lines: Vec<usize>,
    pub constants: ValueArray,
}

impl Chunk {
    pub fn new() -> Box<Chunk> {
        Box::new(Chunk{code:vec![], constants:vec![], lines:vec![]})
    }

    pub fn write(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line)
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

