mod chunk;
mod debug;
mod value;

fn main() {
    println!("Hello, world!");

    let mut chunk = chunk::Chunk::new();

    let constant = chunk.add_constants(1.2) as u8;
    chunk.write(chunk::OpCode::OP_CONSTANT_CODE);
    chunk.write(constant);

    chunk.write(chunk::OpCode::OP_RETURN_CODE);

    debug::disassemble_chunk(&chunk, "test chunk");
}
