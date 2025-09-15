use crate::chunk;
use crate::value;
use crate::value::as_function_object;
use crate::value::print_value;

#[allow(dead_code)]
pub fn disassemble_chunk(chunk: &chunk::Chunk, name: &str) {
    println!("== {} ==", name);

    let mut offset = 0;
    let code_len = chunk.len();
    while offset < code_len {
        offset = disassemble_instruction(chunk, offset);
    }
    // chunk.code.iter().enumerate().for_each(|(offset, &instruction)| {
    //     disassemble_instruction(chunk, offset);
    // });
}

pub fn disassemble_instruction(chunk: &chunk::Chunk, mut offset: usize) -> usize {
    print!("{:08} ", offset);
    if offset > 0 && chunk.read_line_from_offset(offset) == chunk.read_line_from_offset(offset - 1) {
        print!("       | ");
    } else {
        print!("{:08} ", chunk.read_line_from_offset(offset).unwrap());
    }

    let instruction = chunk::OpCode::from_byte(chunk.read_from_offset(offset).unwrap());
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
            | chunk::OpCode::CloseUpvalue
            | chunk::OpCode::Return) => {
            simple_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), offset)
        }
        Some(op) if matches!(op,
            chunk::OpCode::GetLocal
            | chunk::OpCode::SetLocal
            | chunk::OpCode::GetUpvalue
            | chunk::OpCode::SetUpvalue
            | chunk::OpCode::Call) => {
            byte_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), chunk, offset)
        }
        Some(op) if matches!(op, 
            chunk::OpCode::Jump
            | chunk::OpCode::JumpIfFalse
            | chunk::OpCode::JumpIfTrue) => {
            jump_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), 1, chunk, offset)
        }
        Some(op) if matches!(op,
            chunk::OpCode::Loop) => {
            jump_instruction(&chunk::OpCode::byte_to_string(&instruction).to_string(), -1, chunk, offset)
        }
        Some(op) if matches!(op,
            chunk::OpCode::Closure) => {
            let constant = chunk.read_from_offset(offset + 1).unwrap();
            println!("{:<16} {:>4}", "Closure", constant);
            print_value(chunk.get_constant(constant as usize));
            println!();

            let function = as_function_object(chunk.get_constant(constant as usize));
            for _i in 0..(unsafe { &*function }).upvalue_count {
                let is_local = chunk.read_from_offset(offset).unwrap();
                offset += 1;
                let index = chunk.read_from_offset(offset).unwrap();
                offset += 1;
                println!("{:04}            | {}             {}", offset - 2, if is_local == 1 {"local"} else {"upvalue"}, index);
            }
            offset + 2
        }
        _ => {
            println!("Unknown opcode {}", &chunk::OpCode::byte_to_string(&instruction).to_string());/*  */
            offset + 1
        }
    }
}

fn jump_instruction(name: &str, sign: i32, chunk: &chunk::Chunk, offset: usize) -> usize {
    let mut jump_offset = (chunk.read_from_offset(offset + 1).unwrap() as u16) << 8;
    jump_offset |= chunk.read_from_offset(offset + 2).unwrap() as u16;

    let signed_jump = (sign as isize) * (jump_offset as isize);
    let new_jump_offset = (offset as isize + 3 + signed_jump) as usize;

    println!("{:<16} {:>4} -> {:?}", name, offset, new_jump_offset);
    offset + 3
}

fn constant_instruction(name: &str, chunk: &chunk::Chunk, offset: usize) -> usize {
    let constant = chunk.read_from_offset(offset + 1).unwrap();
    print!("{:<16} {:>4} '", name, constant);
    let constant_index = constant as usize;
    value::print_value(&chunk.get_constant(constant_index));
    println!("'");
    offset + 2
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}

fn byte_instruction(name: &str, chunk: &chunk::Chunk, offset: usize) -> usize {
    let slot = chunk.read_from_offset(offset + 1).unwrap();
    println!("{:<16} {:>4}", name, slot);
    offset + 2
}