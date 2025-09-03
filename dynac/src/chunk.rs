use strum_macros::{EnumString, Display};
use crate::value::{Value, ValueArray};
use std::mem::size_of;
use crate::objects::object::GcSize;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
pub enum OpCode {
    Constant,
    Nil,
    True,
    False,
    Equal,
    Greater,
    Less,
    Add,
    Subtract,
    Multiply,
    Divide,
    Not,
    Negate,
    Print,
    Pop,
    DefineGlobal,
    GetGlobal,
    SetGlobal,
    GetLocal,
    SetLocal,
    GetUpvalue,
    SetUpvalue,
    JumpIfFalse,
    JumpIfTrue,
    Jump,
    Loop,
    Call,
    Closure,
    CloseUpvalue,
    Return,
    ImplementTrait,
    StructType,
    StructInstantiate,
    GetField,
    SetField,
    //Unknown(u8),
}

const OPCODE_ARRAY: [Option<OpCode>; 256] = {
    let mut arr = [None; 256];

    arr[OpCode::Constant as u8 as usize] = Some(OpCode::Constant);
    arr[OpCode::Nil as u8 as usize] = Some(OpCode::Nil);
    arr[OpCode::True as u8 as usize] = Some(OpCode::True);
    arr[OpCode::False as u8 as usize] = Some(OpCode::False);
    arr[OpCode::Equal as u8 as usize] = Some(OpCode::Equal);
    arr[OpCode::Greater as u8 as usize] = Some(OpCode::Greater);
    arr[OpCode::Less as u8 as usize] = Some(OpCode::Less);
    arr[OpCode::Add as u8 as usize] = Some(OpCode::Add);
    arr[OpCode::Subtract as u8 as usize] = Some(OpCode::Subtract);
    arr[OpCode::Multiply as u8 as usize] = Some(OpCode::Multiply);
    arr[OpCode::Divide as u8 as usize] = Some(OpCode::Divide);
    arr[OpCode::Not as u8 as usize] = Some(OpCode::Not);
    arr[OpCode::Negate as u8 as usize] = Some(OpCode::Negate);
    arr[OpCode::Print as u8 as usize] = Some(OpCode::Print);
    arr[OpCode::Pop as u8 as usize] = Some(OpCode::Pop);
    arr[OpCode::DefineGlobal as u8 as usize] = Some(OpCode::DefineGlobal);
    arr[OpCode::GetGlobal as u8 as usize] = Some(OpCode::GetGlobal);
    arr[OpCode::SetGlobal as u8 as usize] = Some(OpCode::SetGlobal);
    arr[OpCode::GetLocal as u8 as usize] = Some(OpCode::GetLocal);
    arr[OpCode::SetLocal as u8 as usize] = Some(OpCode::SetLocal);
    arr[OpCode::GetUpvalue as u8 as usize] = Some(OpCode::GetUpvalue);
    arr[OpCode::SetUpvalue as u8 as usize] = Some(OpCode::SetUpvalue);
    arr[OpCode::JumpIfFalse as u8 as usize] = Some(OpCode::JumpIfFalse);
    arr[OpCode::JumpIfTrue as u8 as usize] = Some(OpCode::JumpIfTrue);
    arr[OpCode::Jump as u8 as usize] = Some(OpCode::Jump);
    arr[OpCode::Loop as u8 as usize] = Some(OpCode::Loop);
    arr[OpCode::Call as u8 as usize] = Some(OpCode::Call);
    arr[OpCode::Closure as u8 as usize] = Some(OpCode::Closure);
    arr[OpCode::CloseUpvalue as u8 as usize] = Some(OpCode::CloseUpvalue);
    arr[OpCode::Return as u8 as usize] = Some(OpCode::Return);
    arr[OpCode::ImplementTrait as u8 as usize] = Some(OpCode::ImplementTrait);
    arr[OpCode::StructType as u8 as usize] = Some(OpCode::StructType);
    arr[OpCode::StructInstantiate as u8 as usize] = Some(OpCode::StructInstantiate);
    arr[OpCode::GetField as u8 as usize] = Some(OpCode::GetField);
    arr[OpCode::SetField as u8 as usize] = Some(OpCode::SetField);
    arr
};

#[allow(non_snake_case)]
#[allow(dead_code)]
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

#[derive(Clone)]
pub struct Chunk {
    code: Vec<u8>,
    lines: Vec<usize>,
    constants: ValueArray,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk{code:vec![], constants:vec![], lines:vec![]}
    }

    pub fn write(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line)
    }

    pub fn write_by_offset(&mut self, offset: usize, byte: u8) {
        self.code[offset] = byte
    }

    pub fn read_from_offset(&self, offset: usize) -> Option<u8> {
        self.code.get(offset).cloned()
    }

    pub fn read_line_from_offset(&self, offset: usize) -> Option<usize> {
        self.lines.get(offset).cloned()
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn find_constant(&self, value: Value) -> Option<usize> {
        self.constants.iter().position(|&x| x == value)
    }

    pub fn get_constant(&self, offset: usize) -> &Value {
        self.constants.get(offset).unwrap()
    }

    pub fn len(&self) -> usize {
        self.code.len()
    }

    // For garbage collection - iterate over constants
    pub fn iter_constants(&self) -> impl Iterator<Item = &Value> {
        self.constants.iter()
    }
}

impl GcSize for Chunk {
    #[allow(dead_code)]
    fn shallow_size(&self) -> usize { size_of::<Chunk>() }
    fn deep_size(&self) -> usize {
        // Vec layout already in shallow; add backing buffers via capacity * element size.
        let code_bytes = self.code.capacity() * size_of::<u8>();
        let line_bytes = self.lines.capacity() * size_of::<usize>();
        let constants_bytes = self.constants.capacity() * size_of::<Value>();
        self.shallow_size() + code_bytes + line_bytes + constants_bytes
    }
}

