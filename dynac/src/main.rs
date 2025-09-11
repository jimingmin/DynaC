use std::io::{self, Write};
use std::fs;
use std::process;

mod objects;
mod std_mod;

mod chunk;
mod debug;
mod value;
mod vm;
mod scanner;
mod compiler;
mod table;
mod call_frame;
mod constants;
mod gc;


fn repl() {
    let mut vm = vm::VM::new();
    let mut line = String::new();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        line.clear();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                match vm.interpret(&line) {
                    vm::InterpretResult::InterpretCompileError => process::exit(65),
                    vm::InterpretResult::InterpretRuntimeError => process::exit(70),
                    vm::InterpretResult::InterpretOk => (),
                }
            }
            Err(error) => eprintln!("Error reading line: {}", error),
        }
    }
}

fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Could not read file \"{}\": {}", path, e);
            process::exit(74);
        },
    };

    let mut vm = vm::VM::new();
    match vm.interpret(&source) {
        vm::InterpretResult::InterpretCompileError => process::exit(65),
        vm::InterpretResult::InterpretRuntimeError => process::exit(70),
        vm::InterpretResult::InterpretOk => (),
    }
}

fn main() {
    // let mut chunk = chunk::Chunk::new();

    // let constant = chunk.add_constant(1.2) as u8;
    // chunk.write(chunk::OpCode::Constant as u8, 123);
    // chunk.write(constant, 123);

    // let constant2 = chunk.add_constant(3.4) as u8;
    // chunk.write(chunk::OpCode::Constant as u8, 123);
    // chunk.write(constant2, 123);

    // chunk.write(chunk::OpCode::Add as u8, 123);

    // let constant3 = chunk.add_constant(5.6) as u8;
    // chunk.write(chunk::OpCode::Constant as u8, 123);
    // chunk.write(constant3, 123);

    // chunk.write(chunk::OpCode::Divide as u8, 123);

    // chunk.write(chunk::OpCode::Negate as u8, 123);

    // chunk.write(chunk::OpCode::Return as u8, 123);

    // debug::disassemble_chunk(&chunk, "test chunk");

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 {
        let program = std::path::Path::new(&args[0])
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dynac");
        eprintln!("Usage: {program} <script.dc>");
        process::exit(64);
    } else if args.len() == 2 {
        run_file(&args[1]);
    } else {
        repl();
    }

    
    //vm.interpret(chunk);
}
