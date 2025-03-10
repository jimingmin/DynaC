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

    let instruction = chunk.code[offset];
    match instruction {
        chunk::OpCode::OP_CONSTANT_CODE => {
            constant_instruction("OP_CONSTANT", chunk, offset)
        }
        chunk::OpCode::OP_NEGATE_CODE => {
            simple_instruction("OP_NEGATE", offset)
        }
        chunk::OpCode::OP_RETURN_CODE => {
            simple_instruction("OP_RETURN", offset)
        }
        _ => {
            println!("Unknown opcode {}", instruction);/*  */
            offset + 1
        }
    }
}

fn constant_instruction(name: &str, chunk: &chunk::Chunk, offset: usize) -> usize {
    let constant = chunk.code[offset + 1];
    print!("{:<16} {:>4} '", name, constant);
    let constant_index = constant as usize;
    value::print_value(chunk.constants[constant_index]);
    println!("'");
    offset + 2
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}