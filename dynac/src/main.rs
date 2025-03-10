mod chunk;
mod debug;
mod value;
mod vm;

fn main() {
    let mut chunk = chunk::Chunk::new();

    let constant = chunk.add_constants(1.2) as u8;
    chunk.write(chunk::OpCode::OP_CONSTANT_CODE, 123);
    chunk.write(constant, 123);
    chunk.write(chunk::OpCode::OP_NEGATE_CODE, 123);

    chunk.write(chunk::OpCode::OP_RETURN_CODE, 123);

    debug::disassemble_chunk(&chunk, "test chunk");

    let mut vm = vm::VM::new();
    vm.interpret(chunk);
}
