use crate::chunk;
use crate::value;

pub fn disassemble_chunk(chunk: &chunk::Chunk, name: &str) {
    println!("== {} ==", name);

    let mut offset = 0;
    let code_len = chunk.code.len();
    while offset < code_len {
        offset = disassemble_instruction(chunk, offset);
    }
    // chunk.code.iter().enumerate().for_each(|(offset, &instruction)| {
    //     disassemble_instruction(chunk, offset);
    // });
}

pub fn disassemble_instruction(chunk: &chunk::Chunk, offset: usize) -> usize {
    print!("{:08} ", offset);
    if offset > 0 && chunk.lines[offset] == chunk.lines[offset - 1] {
        print!("       | ");
    } else {
        print!("{:08} ", chunk.lines[offset]);
    }

    let instruction = chunk::OpCode::from_byte(chunk.code[offset]);
    match instruction {
        Some(op) if matches!(op,
            chunk::OpCode::Constant
            | chunk::OpCode::DefineGlobal
            | chunk::OpCode::GetGlobal
            | chunk::OpCode::SetGlobal
        ) => {
            constant_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), chunk, offset)
        }
        Some(op) if matches!(op,
            chunk::OpCode::Nil
            | chunk::OpCode::True
            | chunk::OpCode::False
            | chunk::OpCode::Equal
            | chunk::OpCode::Greater
            | chunk::OpCode::Less
            | chunk::OpCode::Negate
            | chunk::OpCode::Add
            | chunk::OpCode::Subtract
            | chunk::OpCode::Multiply
            | chunk::OpCode::Divide
            | chunk::OpCode::Not
            | chunk::OpCode::Print
            | chunk::OpCode::Pop
            | chunk::OpCode::Return) => {
            simple_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), offset)
        }
        Some(op) if matches!(op,
            chunk::OpCode::GetLocal
            | chunk::OpCode::SetLocal) => {
            byte_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), chunk, offset)
        }
        Some(op) if matches!(op, 
            chunk::OpCode::Jump
            | chunk::OpCode::JumpIfFalse) => {
            jump_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), 1, chunk, offset)
        }
        _ => {
            println!("Unknown opcode {}", &chunk::OpCode::byte_to_string(&instruction).to_string());/*  */
            offset + 1
        }
    }
}

fn jump_instruction(name: &str, sign: i32, chunk: &chunk::Chunk, offset: usize) -> usize {
    let mut jump_offset = (chunk.code[offset + 1] as u16) << 8;
    jump_offset |= chunk.code[offset + 2] as u16;

    let signed_jump = (sign as isize) * (jump_offset as isize);
    let new_jump_offset = (offset as isize + 3 + signed_jump) as usize;

    println!("{:<16} {:>4} -> {:?}", name, offset, new_jump_offset);
    offset + 3
}

fn constant_instruction(name: &str, chunk: &chunk::Chunk, offset: usize) -> usize {
    let constant = chunk.code[offset + 1];
    print!("{:<16} {:>4} '", name, constant);
    let constant_index = constant as usize;
    value::print_value(&chunk.constants[constant_index]);
    println!("'");
    offset + 2
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}

fn byte_instruction(name: &str, chunk: &chunk::Chunk, offset: usize) -> usize {
    let slot = chunk.code[offset + 1];
    println!("{:<16} {:>4}", name, slot);
    offset + 2
}