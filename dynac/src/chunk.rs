use crate::value::{Value, ValueArray};

pub enum OpCode {
    OpConstant,
    OpNegate,
    OpReturn,
    Unknown(u8),
}

impl OpCode {
    pub const OP_RETURN_CODE: u8 = 1;
    pub const OP_CONSTANT_CODE: u8 = 2;
    pub const OP_NEGATE_CODE: u8 = 3;

    pub fn from_byte(byte: u8) -> Option<OpCode> {
        match byte {
            OP_CONSTANT_CODE => Some(OpCode::OpConstant),
            OP_NEGATE_CODE => Some(OpCode::OpNegate),
            OP_RETURN_CODE => Some(OpCode::OpReturn),
            _ => None,
        }
    }

    pub fn to_byte(&self) -> u8 {
        match self {
            OpCode::OpConstant => Self::OP_CONSTANT_CODE,
            OpCode::OpNegate => Self::OP_NEGATE_CODE,
            OpCode::OpReturn => Self::OP_RETURN_CODE,
            OpCode::Unknown(unknown) => *unknown,
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

