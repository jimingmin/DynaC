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
        Some(chunk::OpCode::Constant) => {
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
            | chunk::OpCode::Return) => {
            simple_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), offset)
        }
        _ => {
            println!("Unknown opcode {}", &chunk::OpCode::byte_to_string(&instruction).to_string());/*  */
            offset + 1
        }
    }
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