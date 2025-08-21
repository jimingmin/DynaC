use std::ptr::NonNull;

use crate::{
    call_frame::CallFrame,
    chunk::{self, Chunk},
    compiler::Parser,
    constants::{MAX_FRAMES_SIIZE, MAX_STACK_SIZE},
    debug,
    objects::{
        object::{Object, ObjectType},
        object_closure::ObjectClosure,
        object_function::ObjectFunction,
        object_string::ObjectString,
        object_upvalue::ObjectUpvalue,
    },
    std_mod::time::ClockTime,
    table::Table,
    value::{
        as_bool, as_closure_object, as_function_object, as_native_function_object,
        as_number, as_string_object, is_bool, is_closure, is_function, is_native_function, 
        is_nil, is_number, is_object, is_string, make_bool_value, make_closure_value, make_function_value,
        make_native_function_value, make_nil_value, make_numer_value, make_string_value,
        print_value, Value, ValueType, ValueUnion
    },
};
use crate::objects::object_manager::ObjectManager;

pub struct VM {
    frames: Vec<Box<CallFrame>>,
    stack: [Value; MAX_STACK_SIZE],
    stack_top_pos: usize,
    object_manager: Box<ObjectManager>,
    intern_strings: Box<Table>,
    globals: Box<Table>,
    open_upvalues: Vec<*mut ObjectUpvalue>,
}

#[derive(PartialEq)]
pub enum InterpretResult {
    InterpretOk,
    InterpretCompileError,
    InterpretRuntimeError,
}

impl Drop for VM {
    fn drop(&mut self) {
        // loop {
        //     let object = self.object_manager.pop_object();
        //     if object.is_null() {
        //         break;
        //     }

        //     unsafe {
        //         let object_ptr = Box::from_raw(object);
        //         if object_ptr.obj_type == ObjectType::ObjString {
        //             let object_string = &*(object as *const ObjectString);
        //             println!("VM is droping string object:{}, addr:{:p}", object_string.content, &*object_ptr);
        //         } else if object_ptr.obj_type == ObjectType::ObjFunction {
        //             let object_function = &*(object as *const ObjectFunction);
        //             println!("VM is droping function object:{}", object_function.name);
        //         } else if object_ptr.obj_type == ObjectType::ObjClosure {
        //             println!("VM is droping closure object");
        //         } else if object_ptr.obj_type == ObjectType::ObjUpvalue {
        //             println!("VM is droping upvalue object");
        //         }
        //     }
        // }
        unsafe {
            self.object_manager.free_all();
        }
    }
}

impl VM {
    pub fn new() -> Box<VM> {
        Box::new(VM {
                frames: vec![],
                stack: [Value {
                    value_type: ValueType::ValueNil,
                    value_as: ValueUnion{number: 0.0},
                }; MAX_STACK_SIZE],
                stack_top_pos: 0,
                object_manager: Box::new(ObjectManager::new()),
                intern_strings: Box::new(Table::new()),
                globals: Box::new(Table::new()),
                open_upvalues: vec![],
            })
    }

    pub fn interpret(&mut self, source: &str) -> InterpretResult {
        self.setup_standards();
        self.compile(source)
    }

    fn compile(&mut self, source: &str) -> InterpretResult {
        let mut parser = Box::new(Parser::new(&mut self.object_manager, &mut self.intern_strings));
        if let Some(function_ptr) = parser.compile(source) {
            self.push(make_function_value(function_ptr));

            // let mut frame = Box::new(CallFrame::new(NonNull::new(&mut self.stack[0]).unwrap()));
            // frame.set_function(function.clone());
            // self.frames.push(frame);

            self.call_function(function_ptr, 0);
        } else {
            println!("Compile Error!");
            return InterpretResult::InterpretCompileError;
        }

        // return InterpretResult::InterpretOk;
        match self.run() {
            Ok(result) => result,
            Err(e) => {
                println!("Error during interpretation: {}", e);
                return InterpretResult::InterpretRuntimeError;
            },
        }
    }

    fn setup_standards(&mut self) {
        let clock_ptr = self.object_manager.alloc_native_function("clock".to_string(), 0, ClockTime::new());
        self.globals.insert("clock".to_string(), make_native_function_value(clock_ptr));
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        let current_frame_index = self.frames.len() - 1;
        &mut self.frames[current_frame_index]
    }

    /// Get the current chunk for execution
    /// # Safety
    /// This function is safe because it only dereferences pointers that are guaranteed to be valid:
    /// - The function pointer comes from a valid CallFrame
    /// - The closure.function pointer comes from a valid closure
    unsafe fn current_chunk(&mut self) -> &mut Box<Chunk> {
        match self.current_frame().object_type() {
            ObjectType::ObjFunction => {
                let function = self.current_frame().function();
                &mut (*function).chunk 
            },
            ObjectType::ObjClosure => {
                let closure = self.current_frame().closure();
                &mut (*closure.function).chunk 
            },
            _ => unreachable!()
        }
        
        // RefMut::map(self.current_frame().function(), |f| {
        //     &mut f.chunk
        // })
    }

    fn push(&mut self, value: Value) {
        if self.stack_top_pos < MAX_STACK_SIZE {
            self.stack[self.stack_top_pos] = value;
            self.stack_top_pos += 1;
        } else {
            panic!("Stack overflow");
        }
    }

    fn pop(&mut self) -> Value {
        if self.stack_top_pos > 0 {
            self.stack_top_pos -= 1;
            self.stack[self.stack_top_pos]
        } else {
            panic!("Stack underflow");
        }
    }

    fn peek(&self) -> Option<Value> {
        if self.stack_top_pos > 0 {
            Some(self.stack[self.stack_top_pos - 1])
        } else {
            None
        }
    }

    fn peek_steps(&self, distance: usize) -> Option<Value> {
        if self.stack_top_pos > 0 {
            Some(self.stack[self.stack_top_pos - distance - 1])
        } else {
            None
        }
    }

    fn is_falsey(value: &Value) -> bool {
        is_nil(value) || (is_bool(value) && !as_bool(value))
    }

    fn call_value(&mut self, callee: Value, argument_count: u8) -> bool {
        if is_object(&callee) {
            if is_function(&callee) {
                return self.call_function(as_function_object(&callee) as *mut ObjectFunction, argument_count);
            } else if is_native_function(&callee) {
                let native_function = as_native_function_object(&callee);
                let result = (unsafe { &*native_function }).invoke(&None);
                match result {
                    Ok(value) => {
                        self.stack_top_pos -= (unsafe { &*native_function }).arity as usize + 1;
                        self.push(value);
                        return true;
                    },
                    Err(message) => {
                        let _ = self.runtime_error(&format!("Native function {} has exception {}.", (unsafe { &*native_function }).name, message));
                        return false;
                    }
                }
            } else if is_closure(&callee) {
                let closure_ptr = as_closure_object(&callee) as *mut ObjectClosure;
                return self.call_closure(closure_ptr, argument_count);
            }

        }
        let _ = self.report("Can only call functions and classes.");
        false
    }

    fn call_function(&mut self, function: *mut ObjectFunction, argument_count: u8) -> bool {
        let arity = unsafe { &(*function) }.arity;
        if arity != argument_count {
            let _ = self.runtime_error(format!("Expected {} arguments but got {}.", arity, argument_count).as_str());
            return false;
        }

        if self.frames.len() >= MAX_FRAMES_SIIZE {
            let _ = self.runtime_error("Stack overflow.");
            return false;
        }
        let stack_base_pos = self.stack_top_pos - argument_count as usize - 1;
        let mut frame = CallFrame::new(NonNull::new(&mut self.stack[stack_base_pos]).unwrap(), stack_base_pos);
        // unsafe {
        //     let rc_function = Rc::from_raw(function);
        //     match Rc::try_unwrap(rc_function) {
        //         Ok(f) => {
        //             frame.set_function(Rc::new(RefCell::new(f))); 
        //         },
        //         Err(f) => {
        //             return false;
        //         }
        //     }
        // }
        frame.set_callable_object(function as *mut Object);
        self.frames.push(Box::new(frame));

        true
    }

    fn call_closure(&mut self, closure: *mut ObjectClosure, argument_count: u8) -> bool {
        let function = unsafe { &*(*closure).function };
        let arity = function.arity;
        if arity != argument_count {
            let _ = self.runtime_error(format!("Expected {} arguments but got {}.", arity, argument_count).as_str());
            return false;
        }

        if self.frames.len() >= MAX_FRAMES_SIIZE {
            let _ = self.runtime_error("Stack overflow.");
            return false;
        }
        let stack_base_pos = self.stack_top_pos - argument_count as usize - 1;
        let mut frame = CallFrame::new(NonNull::new(&mut self.stack[stack_base_pos]).unwrap(), stack_base_pos);
        frame.set_callable_object(closure as *mut Object);
        self.frames.push(Box::new(frame));

        true
    }

    fn run(&mut self) -> Result<InterpretResult, String> {
        loop {
            //debug_feature::disassemble_instruction(self);

            let instruction = match self.read_byte() {
                Some(byte) => chunk::OpCode::from_byte(byte),
                None => return self.report("Unexpected end of bytecode"),
            };

            match instruction {
                Some(chunk::OpCode::Constant) => {
                    if let Some(constant) = self.read_constant() {
                        self.push(constant);
                    }
                }
                Some(chunk::OpCode::Nil) => {
                    self.push(make_nil_value());
                }
                Some(chunk::OpCode::True) => {
                    self.push(make_bool_value(true));
                }
                Some(chunk::OpCode::False) => {
                    self.push(make_bool_value(false));
                }
                Some(chunk::OpCode::Equal) => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(make_bool_value(a == b));
                }
                Some(chunk::OpCode::Greater) => {
                    let result = self.binary_op(chunk::OpCode::Greater);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Less) => {
                    let result = self.binary_op(chunk::OpCode::Less);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Add) => {
                    if let Some(value_b) = self.peek_steps(0) {
                        if let Some(value_a) = self.peek_steps(1) {
                            if is_string(&value_a) && is_string(&value_b) {
                                unsafe {
                                    let string_b = &*(as_string_object(&self.pop()));
                                    let string_a = &*(as_string_object(&self.pop()));
                                    let mut combination = String::with_capacity(string_a.content.len() + string_b.content.len());
                                    combination.push_str(string_a.content.as_str());
                                    combination.push_str(string_b.content.as_str());
                                    let combinated_value = make_string_value(&mut self.object_manager, &mut self.intern_strings, combination.as_str());
                                    self.push(combinated_value);
                                }
                            } else if is_number(&value_a) && is_number(&value_b) {
                                let result = self.binary_op(chunk::OpCode::Add);
                                match result {
                                    Err(_) => return result,
                                    _ => (),
                                }
                            } else {
                                return self.report("Operands must be two numbers or two strings.");
                            }
                        } else {
                            return self.report("There is a lack of second operand in the '+' Operation.");
                        }
                    } else {
                        return self.report("There is a lack of operands in the '+' Operation.");
                    }

                    // let result = self.binary_op(chunk::OpCode::Add);
                    // match result {
                    //     Err(_) => return result,
                    //     _ => (),
                    // }
                }
                Some(chunk::OpCode::Subtract) => {
                    let result = self.binary_op(chunk::OpCode::Subtract);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Multiply) => {
                    let result = self.binary_op(chunk::OpCode::Multiply);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Divide) => {
                    let result = self.binary_op(chunk::OpCode::Divide);
                    match result {
                        Err(_) => return result,
                        _ => (),
                    }
                }
                Some(chunk::OpCode::Not) => {
                    let byte = self.pop();
                    self.push(make_bool_value(Self::is_falsey(&byte)));
                }
                Some(chunk::OpCode::Negate) => {
                    if let Some(value) = self.peek_steps(0) {
                        if !is_number(&value) {
                            return self.report("Operand must be a number.");
                        }
                    }
                    let byte = self.pop();
                    let value = make_numer_value(-as_number(&byte));
                    self.push(value);
                }
                Some(chunk::OpCode::Print) => {
                    print_value(&self.pop());
                    println!();
                }
                Some(chunk::OpCode::Pop) => {
                    self.pop();
                }
                Some(chunk::OpCode::DefineGlobal) => {
                    if let Some(object_string) = self.read_string() {
                        if let Some(value) = self.peek() {
                            self.globals.insert((unsafe { (*object_string).clone() }).content.clone(),
                                value);
                            self.pop();
                        } else {
                            return self.report(format!("No value on stack to define the global value {}.", (unsafe { (*object_string).clone() }).content).as_str());
                        }
                    } else {
                        return self.report("Unknown global variable defination.");
                    }
                }
                Some(chunk::OpCode::GetGlobal) => {
                    if let Some(object_string) = self.read_string() {
                        let key = unsafe { &(*object_string).content };
                        if let Some(value) = self.globals.find(key) {
                            self.push(value);
                        } else {
                            return self.report(format!("Undefined global variable {}.", key).as_str());
                        }
                    } else {
                        return self.report("Unknown global variable.");
                    }
                }
                Some(chunk::OpCode::SetGlobal) => {
                    if let Some(object_string) = self.read_string() {
                        if let Some(value) = self.peek() {
                            let key = (unsafe { (*object_string).clone() }).content.clone();
                            if let None = self.globals.insert(key, value) { // It's a new key that means the target key has not been defined.
                                self.globals.remove(&(unsafe { (*object_string).clone() }).content);
                                return self.report("Unknown global variable.");
                            }
                        } else {
                            return self.report(format!("No value on stack to set the global value {}.", (unsafe { (*object_string).clone() }).content).as_str());
                        }
                    } else {
                        return self.report("Unknown global variable.");
                    }
                }
                Some(chunk::OpCode::GetLocal) => {
                    if let Some(slot) = self.read_byte() {
                        let local = *self.current_frame().get_stack_value(slot as usize);
                        self.push(local);
                    } else {
                        return self.report("Unknown local variable.");
                    }
                }
                Some(chunk::OpCode::SetLocal) => {
                    if let Some(slot) = self.read_byte() {
                        if let Some(value) = self.peek() {
                            self.current_frame().set_stack_value(slot as usize, value);
                        } else {
                            return self.report("No value on stack to set the local value.");
                        }
                    } else {
                        return self.report("Unknown local variable.");
                    }
                }
                Some(chunk::OpCode::GetUpvalue) => {
                    let slot = self.read_byte().unwrap();
                    let clousre = self.current_frame().closure();
                    let upvalue_index = *clousre.upvalues.get(slot as usize).unwrap();
                    let upvalue = self.get_upvalue(upvalue_index);
                    self.push(upvalue);
                }
                Some(chunk::OpCode::SetUpvalue) => {
                    let slot = self.read_byte().unwrap();
                    let clousre = self.current_frame().closure();
                    let upvalue_index = *clousre.upvalues.get(slot as usize).unwrap();
                    let value = self.peek().unwrap();
                    self.set_upvalue(upvalue_index, value);
                }
                Some(chunk::OpCode::JumpIfFalse) => {
                    if let Some(offset) = self.read_short() {
                        if let Some(value) = self.peek() {
                            if Self::is_falsey(&value) {
                                *self.current_frame().ip() += offset as usize;
                            }
                        } else {
                            return self.report("No value on stack for condition expression result.");
                        }
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::JumpIfTrue) => {
                    if let Some(offset) = self.read_short() {
                        if let Some(value) = self.peek() {
                            if !Self::is_falsey(&value) {
                                *self.current_frame().ip() += offset as usize;
                            }
                        } else {
                            return self.report("No value on stack for condition expression result.");
                        }
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::Jump) => {
                    if let Some(offset) = self.read_short() {
                        *self.current_frame().ip() += offset as usize;
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::Loop) => {
                    if let Some(offset) = self.read_short() {
                        *self.current_frame().ip() -= offset as usize;
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::Call) => {
                    if let Some(argument_count) = self.read_byte() {
                        if !self.call_value(self.peek_steps(argument_count as usize).unwrap(), argument_count) {
                            return self.report("Instruction Call failed.");
                        }
                        //*self.current_frame().ip() -= argument_count as usize;
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::Closure) => {
                    if let Some(function_index) = self.read_constant() {
                        let object_function = as_function_object(&function_index) as *mut ObjectFunction;
                        let closure_ptr = self.object_manager.alloc_closure(object_function);
                        let upvalue_count = unsafe { (*(*closure_ptr).function).upvalue_count };
                        for _ in 0..upvalue_count {
                            let is_local = self.read_byte().unwrap();
                            let index = self.read_byte().unwrap();
                            if is_local == 0 {
                                let upvalues = &mut self.current_frame().closure().upvalues;
                                let uv_index = upvalues.get(index as usize).unwrap().clone();
                                unsafe { (*closure_ptr).upvalues.push(uv_index); }
                            } else {
                                let slot = unsafe { self.current_frame().get_stack_base().add(index as usize) };
                                let upvalue_index = self.capture_upvalue(slot);
                                unsafe { (*closure_ptr).upvalues.push(upvalue_index); }
                            }
                        }
                        let closure_object_value = make_closure_value(closure_ptr);
                        self.push(closure_object_value);
                    } else {
                        return self.report("There are not enough bytes to read a short.");
                    }
                }
                Some(chunk::OpCode::CloseUpvalue) => {
                    let last = NonNull::new(&mut self.stack[self.stack_top_pos - 1]).unwrap();
                    self.close_upvalues(last);
                    self.pop();
                }
                Some(chunk::OpCode::Return) => {
                    let result = self.pop();
                    let last = *self.current_frame().get_stack_base();
                    self.close_upvalues(last);
                    let stack_top_pos = self.current_frame().get_stack_base_offset();
                    self.frames.pop();
                    if self.frames.is_empty() {
                        self.pop();
                        return Ok(InterpretResult::InterpretOk);
                    }
                    self.stack_top_pos = stack_top_pos;
                    self.push(result);
                    //print_value(&self.pop());
                    //println!();
                    //return Ok(InterpretResult::InterpretOk);
                }
                _ => return self.report("Unknown opcode"),
            }
        }
    }

    fn get_upvalue(&self, index: usize) -> Value {
        let up_ptr = self.open_upvalues[index];
        // up_ptr must be valid and point to an ObjectUpvalue owned by ObjectManager;
        // it may point to either a stack slot (location) or the upvalue.closed (after closing).
        unsafe {
            let loc = (*up_ptr).location;
            *loc
        }
    }
    fn set_upvalue(&mut self, index: usize, value: Value) {
        let up_ptr = self.open_upvalues[index];
        unsafe {
            let loc = (*up_ptr).location;
            *loc = value;
        }
    }

    fn read_short(&mut self) -> Option<u16> {
        let mut result = None;
        {
            let frame = self.current_frame();
            let ip = *frame.ip();
            let chunk = unsafe { self.current_chunk() };
            
            if ip + 1 < chunk.len() {
                let short = ((chunk.read_from_offset(ip).unwrap() as u16) << 8) |
                    chunk.read_from_offset(ip + 1).unwrap() as u16;
                result = Some(short);
            }
        }
        if result.is_some() {
            *self.current_frame().ip() += 2;
        }
        result
    }

    fn read_byte(&mut self) -> Option<u8> {
        let mut result = None;
        {
            let frame = self.current_frame();
            let ip = *frame.ip();
            let chunk = unsafe { self.current_chunk() };

            if ip < chunk.len() {
                result = chunk.read_from_offset(ip);
            }
        }
        if result.is_some() {
            *self.current_frame().ip() += 1;
        }

        result
    }

    fn read_constant(&mut self) -> Option<Value> {
        let instruction = match self.read_byte() {
            Some(byte) => byte,
            None => return None,
        };
        let chunk = unsafe { self.current_chunk() };
        Some(*chunk.get_constant(instruction as usize))
    }

    fn read_string(&mut self) -> Option<*const ObjectString> {
        if let Some(constant) = self.read_constant() {
            Some(as_string_object(&constant))
        } else {
            None
        }
    }

    // fn BinaryOperation(&mut self, op_code: chunk::OpCode) {
    //     let a = self.pop();
    //     let b = self.pop();
    //     match op_code {
    //         chunk::OpCode::Add => self.push(a + b),
    //         chunk::OpCode::Subtract => self.push(a - b),
    //         chunk::OpCode::Multiply => self.push(a * b),
    //         chunk::OpCode::Divide => self.push(a / b),
    //         _ => panic!("Unknown binary operator"),
    //     }
    // }

    fn binary_op(
        &mut self,
        op_code: chunk::OpCode,
    ) -> Result<InterpretResult, String> {
            if self.stack_top_pos < 2 {
                return self.report("Binary operator must have two operands.");
            }

            if let Some(b) = self.peek_steps(0) {
                if !is_number(&b) {
                    return self.report("Second operand must be a number.");
                }
            }

            if let Some(a) = self.peek_steps(1) {
                if !is_number(&a) {
                    return self.report("First operand must be a number.");
                }
            }
            let value_b = as_number(&self.pop());
            let value_a = as_number(&self.pop());
            match op_code {
                chunk::OpCode::Greater => {
                    self.push(make_bool_value(value_a > value_b))
                }
                chunk::OpCode::Less => {
                    self.push(make_bool_value(value_a < value_b))
                }
                chunk::OpCode::Add => {
                    self.push(make_numer_value(value_a + value_b))
                }
                chunk::OpCode::Subtract => {
                    self.push(make_numer_value(value_a - value_b))
                }
                chunk::OpCode::Multiply => {
                    self.push(make_numer_value(value_a * value_b))
                }
                chunk::OpCode::Divide => {
                    self.push(make_numer_value(value_a / value_b))
                }
                _ => return self.report("Unknown binary operator."),
            };

            Ok(InterpretResult::InterpretOk)
    }

    fn capture_upvalue(&mut self, slot: NonNull<Value>) -> usize {
        // let mut target_index = 0;
        // for (index, value) in self.open_upvalues.iter_mut().enumerate().rev() {
        //     if slot == *value.location() {
        //         target_index = index;
        //         break;
        //     } else if slot > *value.location() {
        //         target_index = self.open_upvalues.len() - index;
        //         self.open_upvalues.insert(target_index, ObjectUpvalue::new(slot));
        //         break;
        //     }
        // }
        // if self.open_upvalues.is_empty() {
        //     self.open_upvalues.insert(target_index, ObjectUpvalue::new(slot));
        // }
        // target_index
        let slot_ptr = slot.as_ptr();
        // find existing upvalue
        for (i, &up_ptr) in self.open_upvalues.iter().enumerate() {
            let loc = unsafe { (*up_ptr).location };
            if loc == slot_ptr {
                return i;
            }
        }
        // not found -> allocate a new upvalue via ObjectManager (heap stable) and push pointer
        let new_up = self.object_manager.alloc_upvalue(slot_ptr);
        self.open_upvalues.push(new_up);
        self.open_upvalues.len() - 1
    }

    fn close_upvalues(&mut self, last: NonNull<Value>) {
        // for value in self.open_upvalues.iter_mut().enumerate().rev() {
        //     if value.1.location < last {
        //         break;
        //     }

        //     value.1.closed = unsafe { value.1.location.as_ref().deep_clone() };//unsafe { *value.1.location.as_ptr().clone() };
        //     let v = &mut value.1.closed;
        //     value.1.location = NonNull::new(v).unwrap();
        // }
       let last_ptr = last.as_ptr();
       // iterate in reverse and close those whose location >= last_ptr (stack grows upward assumption)
       for i in (0..self.open_upvalues.len()).rev() {
           let up_ptr = self.open_upvalues[i];
           let loc = unsafe { (*up_ptr).location };
           if loc < last_ptr {
               break;
           }
           unsafe {
               // copy the value from the stack into the upvalue.closed field
               (*up_ptr).closed = *loc;
               // point location to the closed field inside the upvalue (stable because upvalue is heap allocated)
               (*up_ptr).location = &mut (*up_ptr).closed as *mut Value;
           }
       }
    }

    fn report(&mut self, message: &str) -> Result<InterpretResult, String> {
        self.report_runtime_error(message)
    }

    fn report_runtime_error(&mut self, message: &str) -> Result<InterpretResult, String> {
        self.runtime_error(message)
    }

    fn runtime_error(&mut self, message: &str) -> Result<InterpretResult, String> {
        // Print the formatted error message to stderr
        //eprintln!("{}", args);

        // Calculate instruction offset
        //unsafe {
            let frame = self.current_frame();
            let instruction_index = *frame.ip() - 1;
            let chunk = unsafe { self.current_chunk() };
            if let Some(instruction) = chunk.read_from_offset(instruction_index) {
                if let Some(line) = chunk.read_line_from_offset(instruction as usize) {
                    //eprintln!("[line {}] in script", line);
                    return Err(format_args!("Runtime error: {} [line {}] in script", message, line).to_string());
                } else {
                    return Err(format_args!("Runtime error: {} [line ???] in script (invalid instruction index)", message).to_string());
                    //eprintln!("[line ???] in script (invalid instruction index)");
                }
            } else {
                return Err(format_args!("Runtime error: {} [instruction ???] in script (invalid instruction)", message).to_string());
                //eprintln!("[instruction ???] in script (invalid instruction)");
            }
            //let instruction = (vm.ip as usize) - (vm.chunk as *const _ as *const u8 as usize) - 1;
            
            // Get the corresponding line number
            // if let Some(line) = vm.chunk.as_ref().and_then(|c| c.lines.get(instruction)) {
            //     eprintln!("[line {}] in script", line);
            // } else {
            //     eprintln!("[line ???] in script (invalid instruction index)");
            // }
        //}

        //Err(format!("Runtime error: {}", format))
    }
}

#[cfg(feature = "debug_trace_execution")]
mod debug_feature {
    use super::*;

    #[allow(dead_code)]
    pub fn disassemble_instruction(vm: &mut VM) {
        if vm.stack_top_pos < 1 {
            return;
        }

        print!("{: >17}", "");
        for slot in &vm.stack[0..vm.stack_top_pos] {
            print!(" [ ");
            print_value(slot);
            print!(" ]");
        }
        println!();
        let ip = *vm.current_frame().ip();
        debug::disassemble_instruction(unsafe { vm.current_chunk() }.as_ref(), ip);
    }
}

#[cfg(not(feature = "debug_trace_execution"))]
mod debug_feature {
    use super::*;

    pub fn disassemble_instruction(vm: &VM) {}
}


#[cfg(test)]
mod tests {
    use crate::vm::InterpretResult;

    use super::VM;

    #[test]
    fn test_comparison_expression() {
        let mut vm = VM::new();
        assert!(vm.interpret("!(5 - 4 > 3 * 2 == !nil);") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_string_concatenate() {
        let mut vm = VM::new();
        assert!(vm.interpret("\"st\" + \"ri\" + \"ng\";") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret("print 1 + 2; print 3 * 4;") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_global_var() {
        let mut vm = VM::new();
        assert!(vm.interpret("var beverage = \"coffee\"; 
                            var breakfast = \"beignets with \" + beverage;
                            print breakfast;") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_define_global_var() {
        let mut vm = VM::new();
        assert!(vm.interpret("var beverage = \"coffee\";") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_local_var() {
        let mut vm = VM::new();
        assert!(vm.interpret("{var a = \"hello world!\"; a = \"111\"; print a;}") == InterpretResult::InterpretOk);
        assert!(vm.interpret("{
                                var a = \"the first\";
                                {
                                    var a = \"the second\";
                                    print a;
                                }
                                print a;
                            }") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_if_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret("print \"test if statement...\";
                            if (1 > 0) {
                                print \"'1 > 0' is true\";
                            }") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_else_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret("print \"test else clause...\";
                            if (1 < 0) {
                                print \"'1 < 0' is true\";
                            } else {
                                print \"'1 < 0' is false\";
                            }") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_and_operator() {
        let mut vm = VM::new();
        assert!(vm.interpret("print \"test and operator...\";
                            if ( 1 > 0 and 2 > 1) {
                                print \"'1 > 0 and 2 > 1' is true\";
                            } else {
                                print \"'1 > 0 and 2 > 1' is false\";
                            }
                            
                            if ( 1 > 0 and 2 < 1) {
                                print \"'1 > 0 and 2 < 1' is true\";
                            } else {
                                print \"'1 > 0 and 2 < 1' is false\";
                            }") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_or_operator() {
        let mut vm = VM::new();
        assert!(vm.interpret("print \"test or operator...\";
                            if ( 1 > 0 or 2 > 1) {
                                print \"'1 > 0 or 2 > 1' is true\";
                            } else {
                                print \"'1 > 0 or 2 > 1' is false\";
                            }
                            
                            if ( 1 > 0 or 2 < 1) {
                                print \"'1 > 0 or 2 < 1' is true\";
                            } else {
                                print \"'1 > 0 or 2 < 1' is false\";
                            }") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_while_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret("print \"test while statement...\";
                            var count = 1;
                            while (count > 0) {
                                print count;
                                count = count - 1;
                            }") == InterpretResult::InterpretOk);
    }
 
    #[test]
    fn test_for_statement() {
        let mut vm = VM::new();
        let result = vm.interpret("print \"test for statement...\";
                            for(var i = 0; i < 2; i = i + 1) {
                                print i;
                            }
                            var i = 0;
                            for (; i < 2; i = i + 1) {
                                print i;
                            }
                            i = 0;
                            for (; i < 1;) {
                                print i;
                                i = i + 1;
                            }");
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_function_call() {
        let mut vm = VM::new();
        let result = vm.interpret(
            "fn sum(a, b, c) {
                        return a + b + c;
                    }
                    print 4 + sum(5, 6, 7);");
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_native_function_call() {
        let mut vm = VM::new();
        let result = vm.interpret(
            "print clock();");
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_fib_function() {
        let mut vm = VM::new();
        let result = vm.interpret("
            fn fib(number) {
                if (number < 2) {
                    return number;
                }

                return fib(number - 2) + fib(number - 1);
            }
            
            var start = clock();
            var result = fib(5);
            print result;
            var end = clock();
            print end - start;");
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_closure() {
        let mut vm = VM::new();
        let result = vm.interpret("
            fn outer() {
                var x = \"outside\";
                fn inner() {
                    print x;
                }
                return inner;
            }
            var closure = outer();
            closure();");
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_closure_with_shared_variable() {
        let mut vm = VM::new();
        let result = vm.interpret("
            var globalSet;
            var globalGet;

            fn main() {
                var a = \"initial\";

                fn set(value) { a = value; }
                fn get() { print a; }

                globalSet = set;
                globalGet = get;
            }

            main();
            globalSet(\"updated\");
            globalGet();
            globalSet(\"initial\");
            globalGet();");
        assert!(result == InterpretResult::InterpretOk);
    }    
}