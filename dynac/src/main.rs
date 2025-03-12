mod chunk;
mod debug;
mod value;
mod vm;

fn main() {
    let mut chunk = chunk::Chunk::new();

    let constant = chunk.add_constants(1.2) as u8;
    chunk.write(chunk::OpCode::OpConstant as u8, 123);
    chunk.write(constant, 123);

    let constant2 = chunk.add_constants(3.4) as u8;
    chunk.write(chunk::OpCode::OpConstant as u8, 123);
    chunk.write(constant2, 123);

    chunk.write(chunk::OpCode::OpAdd as u8, 123);

    let constant3 = chunk.add_constants(5.6) as u8;
    chunk.write(chunk::OpCode::OpConstant as u8, 123);
    chunk.write(constant3, 123);

    chunk.write(chunk::OpCode::OpDivide as u8, 123);

    chunk.write(chunk::OpCode::OpNegate as u8, 123);

    chunk.write(chunk::OpCode::OpReturn as u8, 123);

    debug::disassemble_chunk(&chunk, "test chunk");

    let mut vm = vm::VM::new();
    vm.interpret(chunk);
}
