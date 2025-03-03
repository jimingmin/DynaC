use crate::value::{Value, ValueArray};

pub enum OpCode {
    OpConstant,
    OpReturn,
    Unknown(u8),
}

impl OpCode {
    pub const OP_RETURN_CODE: u8 = 1;
    pub const OP_CONSTANT_CODE: u8 = 2;

    pub fn to_byte(&self) -> u8 {
        match self {
            OpCode::OpConstant => Self::OP_CONSTANT_CODE,
            OpCode::OpReturn => Self::OP_RETURN_CODE,
            OpCode::Unknown(unknown) => *unknown,
        }
    }
}

pub struct Chunk {
    pub count: u32,
    pub capacity: u32,
    pub code: Vec<u8>,
    pub constants: ValueArray,
}

impl Chunk {
    pub fn new() -> Box<Chunk> {
        Box::new(Chunk{count:0, capacity:0, code:vec![], constants:vec![]})
    }

    // fn initChunk(&mut self) {
    //     self.count = 0;
    //     self.capacity = 0;
    // }

    pub fn write(&mut self, byte: u8) {
        self.code.push(byte)
    }

    pub fn add_constants(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

