use std::ptr::NonNull;

use crate::{
    gc::GarbageCollector,
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
        print_value, Value
    },
};
use crate::objects::object_manager::ObjectManager;
use crate::objects::object_struct::{ObjectStructType, ObjectStructInstance};
use std::collections::HashMap;

pub struct VM {
    frames: Vec<Box<CallFrame>>,
    stack: [Value; MAX_STACK_SIZE],
    stack_top_pos: usize,
    object_manager: Box<ObjectManager>,
    intern_strings: Box<Table>,
    globals: Box<Table>,
    struct_types: Box<Table>,
    trait_registry: Box<Table>, // name -> trait object
    // Method registry: type name -> Table(method name -> function/closure value)
    type_methods: HashMap<String, Table>,
    open_upvalues: Vec<*mut ObjectUpvalue>,
    gc: GarbageCollector,
    bytes_allocated: usize,
    next_gc_bytes: usize,
    // Stack struct arenas per frame index (aligned with frames vector indices)
    frame_stack_structs: Vec<Vec<StackStruct>>, // parallel to frames; index = frames.len()-1 current
}

// Non-GC managed stack struct representation
struct StackStruct {
    struct_type: *mut ObjectStructType,
    fields: Vec<Value>,
}

#[derive(PartialEq, Debug)]
pub enum InterpretResult {
    InterpretOk,
    InterpretCompileError,
    InterpretRuntimeError,
}

impl Drop for VM {
    fn drop(&mut self) {
        unsafe {
            self.object_manager.free_all();
        }
    }
}

impl VM {
    pub fn new() -> VM {
        const INITIAL_GC_THRESHOLD: usize = 1024 * 1024; // 1MB
        let vm = VM {
                stack: [Value::new(); MAX_STACK_SIZE],
                stack_top_pos: 0,
                frames: Vec::with_capacity(MAX_FRAMES_SIIZE),
                object_manager: Box::new(ObjectManager::new()),
                intern_strings: Box::new(Table::new()),
                globals: Box::new(Table::new()),
                struct_types: Box::new(Table::new()),
                trait_registry: Box::new(Table::new()),
                type_methods: HashMap::new(),
                open_upvalues: Vec::new(),
                gc: GarbageCollector::new(),
                bytes_allocated: 0,
                next_gc_bytes: INITIAL_GC_THRESHOLD,
                frame_stack_structs: Vec::new(),
            };
        vm
    }
        
    pub fn interpret(&mut self, source: &str) -> InterpretResult {
        self.setup_standards();
        self.compile(source)
    }

    fn compile(&mut self, source: &str) -> InterpretResult {
        let mut parser = Box::new(Parser::new(&mut self.object_manager, &mut self.intern_strings));
        if let Some(function_ptr) = parser.compile(source) {
            self.push(make_function_value(function_ptr));

            self.call_function(function_ptr, 0);
        } else {
            println!("Compile Error!");
            return InterpretResult::InterpretCompileError;
        }

        // Incorporate any allocations performed during compilation (strings, functions) before execution
        self.sync_pending_allocations();
        match self.run() {
            Ok(result) => result,
            Err(e) => {
                println!("Error during interpretation: {}", e);
                return InterpretResult::InterpretRuntimeError;
            },
        }
    }

    fn sync_pending_allocations(&mut self) {
        let new_bytes = self.object_manager.drain_pending_bytes();
        if new_bytes > 0 { self.track_allocation(new_bytes); }
    }

    fn track_allocation(&mut self, bytes: usize) {
        self.bytes_allocated += bytes;
        if self.bytes_allocated > self.next_gc_bytes {
            self.collect_garbage();
        }
    }

    // Test-only helper: allow tests to lower GC threshold to force cycles under smaller workloads.
    #[cfg(test)]
    fn set_gc_threshold(&mut self, threshold: usize) {
        self.next_gc_bytes = threshold;
    }

    fn update_next_gc_threshold(&mut self) {
        // Common GC tuning: increase threshold by a factor (here 2x)
        // This provides a balance between GC frequency and memory usage
        self.next_gc_bytes = self.bytes_allocated * 2;
    }

    fn collect_garbage(&mut self) {
        let before = self.bytes_allocated;
        // Prepare GC
        self.gc.prepare_collection(&self.object_manager);

        // Mark roots
        self.gc.mark_roots(
            &self.stack,
            self.stack_top_pos,
            &self.globals,
            &self.intern_strings,
            &self.frames,
            &self.open_upvalues,
        );

        // Also mark values held inside stack-allocated struct arenas so their referenced
        // heap objects are treated as roots during this GC cycle.
        for arena in &self.frame_stack_structs {
            for st in arena {
                for field in &st.fields { self.gc.mark_value(field); }
            }
        }

        // Mark trait registry values (trait objects)
        for (_, v) in self.trait_registry.iter() { self.gc.mark_value(v); }
    // Mark method tables for each type
    for (_t, tbl) in self.type_methods.iter() { for (_k, v) in tbl.iter() { self.gc.mark_value(v); } }

        // Trace
        self.gc.trace_references();

        // Sweep
        let freed_bytes = self.gc.sweep(&mut self.object_manager);
        self.bytes_allocated = self.bytes_allocated.saturating_sub(freed_bytes);
        self.update_next_gc_threshold();
        let after = self.bytes_allocated;
        let next = self.next_gc_bytes;
        // Record stats cycle
        self.gc.record_cycle(before, freed_bytes, after, next);

        #[cfg(feature = "gc_debug")]
        eprintln!(
            "[gc] cycle done: freed={} bytes before={}KB after={}KB next_trigger={}KB",
            freed_bytes,
            before / 1024,
            self.bytes_allocated / 1024,
            self.next_gc_bytes / 1024
        );
    }

    #[inline]
    fn warn(&self, message: &str) {
        eprintln!("[warn] {}", message);
    }

    fn setup_standards(&mut self) {
        // Root ordering: Insert the newly allocated native function into a root (globals) BEFORE tracking
        // the allocation, because tracking may immediately trigger GC.
        let (clock_ptr, size) = self.object_manager.alloc_native_function("clock".to_string(), 0, ClockTime::new());
        self.globals.insert("clock".to_string(), make_native_function_value(clock_ptr));
        self.track_allocation(size);
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
        if distance < self.stack_top_pos {
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
        self.frame_stack_structs.push(Vec::new()); // new frame stack struct arena

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
        self.frame_stack_structs.push(Vec::new());

        true
    }

    fn run(&mut self) -> Result<InterpretResult, String> {
        loop {
            // Account for any new allocations done since last iteration (e.g., string interning during concatenation)
            self.sync_pending_allocations();
            // (optional) enable disassembly via feature flag: debug_trace_execution

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
                    if self.stack_top_pos < 2 { return self.report("There is a lack of operands in the '+' Operation."); }
                    let value_b = self.peek_steps(0).unwrap();
                    let value_a = self.peek_steps(1).unwrap();
                    if is_string(&value_a) && is_string(&value_b) {
                        unsafe {
                            // preserve ordering: a then b
                            let string_b_ptr = as_string_object(&value_b);
                            let string_a_ptr = as_string_object(&value_a);
                            let string_b = &*string_b_ptr;
                            let string_a = &*string_a_ptr;
                            // pop two values (b then a) from stack
                            self.pop(); // b
                            self.pop(); // a
                            let mut combination = String::with_capacity(string_a.content.len() + string_b.content.len());
                            combination.push_str(string_a.content.as_str());
                            combination.push_str(string_b.content.as_str());
                            let combinated_value = make_string_value(&mut self.object_manager, &mut self.intern_strings, combination.as_str());
                            self.push(combinated_value);
                        }
                    } else if is_number(&value_a) && is_number(&value_b) {
                        let result = self.binary_op(chunk::OpCode::Add);
                        match result { Err(_) => return result, _ => (), }
                    } else {
                        return self.report("Operands must be two numbers or two strings.");
                    }

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
                            // Promote stack struct if necessary when defining a global
                            let promoted = self.promote_stack_struct_value_reason(value, Some("global assignment"), 0);
                            if promoted.value_type != value.value_type { // replaced
                                // overwrite top of stack with promoted heap instance
                                self.stack[self.stack_top_pos - 1] = promoted;
                            }
                            self.globals.insert((unsafe { (*object_string).clone() }).content.clone(),
                                self.peek().unwrap());
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
                            // Promote if needed
                            let promoted = self.promote_stack_struct_value_reason(value, Some("global assignment"), 0);
                            if promoted.value_type != value.value_type {
                                self.stack[self.stack_top_pos - 1] = promoted;
                            }
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
                Some(chunk::OpCode::Invoke) => {
                    // Layout: Invoke <method_name_const_index> <arg_count>
                    let method_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed Invoke (missing method index)") } as usize;
                    let arg_count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed Invoke (missing arg count)") };
                    // Callee is receiver at distance arg_count from top (like Call)
                    let receiver = self.peek_steps(arg_count as usize).unwrap();
                    // Determine type name for method table lookup
                    let type_name = match receiver.value_type {
                        crate::value::ValueType::ValueObject => {
                            let obj_ptr = unsafe { receiver.value_as.object };
                            let obj = unsafe { &*obj_ptr };
                            if obj.obj_type != ObjectType::ObjStructInstance { return self.report("Invoke receiver must be struct instance"); }
                            let inst_ptr = obj_ptr as *mut ObjectStructInstance;
                            let stype_ptr = unsafe { (*inst_ptr).struct_type };
                            unsafe { (*stype_ptr).name.clone() }
                        }
                        crate::value::ValueType::ValueStackStruct => {
                            let idx = unsafe { receiver.value_as.stack_index };
                            let arena = match self.frame_stack_structs.last() { Some(a) => a, None => return self.report("Missing frame arena") };
                            if idx >= arena.len() { return self.report("Invalid stack struct index"); }
                            let s = &arena[idx];
                            unsafe { (*s.struct_type).name.clone() }
                        }
                        _ => return self.report("Invoke receiver must be object or stack struct"),
                    };
                    // Resolve method function
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let mval = unsafe { *(*chunk_ptr).get_constant(method_index) };
                    if !is_string(&mval) { return self.report("Invoke method name constant not string"); }
                    let mname = unsafe { (*as_string_object(&mval)).content.clone() };
                    match self.type_methods.get(type_name.as_str()) {
                        Some(table) => {
                            match table.find(mname.as_str()) {
                                Some(func_val) => {
                                    // Stack layout before: [..., receiver, arg1, ..., argN]
                                    // Insert callee before receiver so layout becomes: [..., callee, receiver, arg1, ..., argN]
                                    let insert_pos = self.stack_top_pos - arg_count as usize - 1;
                                    if self.stack_top_pos >= MAX_STACK_SIZE { return self.report("Stack overflow during invoke"); }
                                    // make room
                                    let old_top = self.stack_top_pos;
                                    self.stack_top_pos += 1;
                                    // shift right
                                    let mut i = old_top;
                                    while i > insert_pos { self.stack[i] = self.stack[i-1]; i -= 1; }
                                    // insert callee
                                    self.stack[insert_pos] = func_val;
                                    // include receiver as first arg
                                    let new_argc = arg_count + 1;
                                    if !self.call_value(func_val, new_argc) { return self.report("Invoke call failed"); }
                                }
                                None => return self.report(format!("Unknown method '{}' for type '{}'", mname, type_name).as_str()),
                            }
                        }
                        None => return self.report(format!("No methods registered for type '{}'", type_name).as_str()),
                    }
                }
                Some(chunk::OpCode::Closure) => {
                    if let Some(function_index) = self.read_constant() {
                        let object_function = as_function_object(&function_index) as *mut ObjectFunction;
                        let (closure_ptr, size) = self.object_manager.alloc_closure(object_function);
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
                        // Push closure onto stack BEFORE accounting bytes to ensure it is marked as a root
                        self.push(closure_object_value);
                        self.track_allocation(size);
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
                    // If returning a stack struct created in this frame -> runtime error (per spec unknown behavior -> forbid)
                    if result.value_type == crate::value::ValueType::ValueStackStruct {
                        // Disallow returning frame-local stack struct
                        return self.report("Cannot return stack-allocated struct; use 'new' to allocate on heap");
                    }
                    let last = *self.current_frame().get_stack_base();
                    self.close_upvalues(last);
                    let stack_top_pos = self.current_frame().get_stack_base_offset();
                    self.frames.pop();
                    self.frame_stack_structs.pop(); // drop arena for this frame
                    if self.frames.is_empty() {
                        self.pop();
                        return Ok(InterpretResult::InterpretOk);
                    }
                    self.stack_top_pos = stack_top_pos;
                    self.push(result);
                }
                Some(chunk::OpCode::ImplementTrait) => {
                    // Layout emitted: ImplementTrait <trait_name_const_index> <method_count> <method_name_const_index>...
                    let name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplementTrait (missing name index)") } as usize;
                    let method_count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplementTrait (missing method count)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let name_val = unsafe { *(*chunk_ptr).get_constant(name_index) };
                    let mut methods: Vec<String> = Vec::with_capacity(method_count);
                    for _ in 0..method_count {
                        let mi = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplementTrait (missing method name index)") } as usize;
                        let mv = unsafe { *(*chunk_ptr).get_constant(mi) };
                        if !is_string(&mv) { return self.report("Trait method name constant not string"); }
                        methods.push(unsafe { (*as_string_object(&mv)).content.clone() });
                    }
                    // Accept either a trait object constant or a name string constant
                    if is_object(&name_val) && unsafe { (*name_val.value_as.object).obj_type } == ObjectType::ObjTrait {
                        let tptr = unsafe { name_val.value_as.object as *mut crate::objects::object_trait::ObjectTrait };
                        let tname = unsafe { (*tptr).name.clone() };
                        unsafe { (*tptr).method_names = methods; }
                        self.trait_registry.insert(tname, name_val);
                    } else if is_string(&name_val) {
                        let trait_name = unsafe { (*as_string_object(&name_val)).content.clone() };
                        if self.trait_registry.find(trait_name.as_str()).is_none() {
                            let (tptr, size) = self.object_manager.alloc_trait(trait_name.clone());
                            unsafe { (*tptr).method_names = methods; }
                            self.trait_registry.insert(trait_name, Value { value_type: crate::value::ValueType::ValueObject, value_as: crate::value::ValueUnion { object: tptr as *mut crate::objects::object::Object } });
                            self.track_allocation(size);
                        }
                    } else { return self.report("ImplementTrait constant must be trait object or name string"); }
                }
                Some(chunk::OpCode::ImplRegister) => {
                    // Layout: ImplRegister <trait_name_idx> <type_name_idx> <method_count> then pairs: <method_name_idx> <function_const_idx>
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let trait_idx = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplRegister (missing trait index)") } as usize;
                    let type_idx = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplRegister (missing type index)") } as usize;
                    let count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplRegister (missing method count)") } as usize;
                    let trait_val = unsafe { *(*chunk_ptr).get_constant(trait_idx) };
                    let type_val = unsafe { *(*chunk_ptr).get_constant(type_idx) };
                    if !is_string(&trait_val) || !is_string(&type_val) { return self.report("ImplRegister expects string constants"); }
                    let trait_name = unsafe { (*as_string_object(&trait_val)).content.clone() };
                    let type_name = unsafe { (*as_string_object(&type_val)).content.clone() };
                    // Ensure trait exists
                    if self.trait_registry.find(trait_name.as_str()).is_none() { return self.report("ImplRegister references unknown trait"); }
                    // Collect entries first to avoid borrowing self during reads
                    let mut entries: Vec<(String, Value)> = Vec::with_capacity(count);
                    for _ in 0..count {
                        let mname_idx = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplRegister (missing method name index)") } as usize;
                        let fn_idx = match self.read_byte() { Some(b) => b, None => return self.report("Malformed ImplRegister (missing function const index)") } as usize;
                        let mname_val = unsafe { *(*chunk_ptr).get_constant(mname_idx) };
                        let fval = unsafe { *(*chunk_ptr).get_constant(fn_idx) };
                        if !is_string(&mname_val) { return self.report("ImplRegister method name not string"); }
                        // Accept only object functions/closures; ignore placeholders
                        if !is_object(&fval) { continue; }
                        entries.push((unsafe { (*as_string_object(&mname_val)).content.clone() }, fval));
                    }
                    let table = self.type_methods.entry(type_name.clone()).or_insert_with(Table::new);
                    for (mn, fv) in entries { table.insert(mn, fv); }
                }
                Some(chunk::OpCode::StructType) => {
                    // Layout: StructType <name_const_index> <field_count> <field_name_const_index>*
                    let name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructType (missing name index)") } as usize;
                    let field_count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructType (missing field count)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let name_value = unsafe { *(*chunk_ptr).get_constant(name_index) };
                    if !is_string(&name_value) { return self.report("StructType name constant not string"); }
                    // Collect field names
                    let mut field_names: Vec<String> = Vec::with_capacity(field_count);
                    for _ in 0..field_count {
                        let fi = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructType (missing field name index)") } as usize;
                        let fv = unsafe { *(*chunk_ptr).get_constant(fi) };
                        if !is_string(&fv) { return self.report("StructType field name constant not string"); }
                        let fname = unsafe { (*as_string_object(&fv)).content.clone() };
                        field_names.push(fname);
                    }
                    // If already registered, ignore (redefinition warning could be added later)
                    unsafe {
                        let struct_name = (*as_string_object(&name_value)).content.clone();
                        if self.struct_types.find(struct_name.as_str()).is_none() {
                            let (stype_ptr, size) = self.object_manager.alloc_struct_type(struct_name.clone());
                            for fname in field_names.iter() {
                                (*stype_ptr).field_index.insert(fname.clone(), make_numer_value((*stype_ptr).field_names.len() as f64));
                                (*stype_ptr).field_names.push(fname.clone());
                            }
                            // store registry value (struct type object) in struct_types table
                            self.struct_types.insert(struct_name, Value { value_type: crate::value::ValueType::ValueObject, value_as: crate::value::ValueUnion { object: stype_ptr as *mut crate::objects::object::Object } });
                            self.track_allocation(size);
                        }
                    }
                }
                Some(chunk::OpCode::StructInstantiate) => {
                    // Layout emitted by compiler: StructInstantiate <type_name_const_index> <field_count> <field_name_const_index>* then field values already on stack in order of appearance
                    let type_name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiate (missing type name index)") } as usize;
                    let field_count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiate (missing field count)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let type_name_value = unsafe { *(*chunk_ptr).get_constant(type_name_index) };
                    if !is_string(&type_name_value) { return self.report("StructInstantiate type name constant not string"); }
                    let mut literal_field_names: Vec<String> = Vec::with_capacity(field_count);
                    for _ in 0..field_count {
                        let fi = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiate (missing field name const index)") } as usize;
                        let fv = unsafe { *(*chunk_ptr).get_constant(fi) };
                        if !is_string(&fv) { return self.report("StructInstantiate field name constant not string"); }
                        let fname = unsafe { (*as_string_object(&fv)).content.clone() };
                        literal_field_names.push(fname);
                    }
                    let struct_name = unsafe { (*as_string_object(&type_name_value)).content.clone() };
                    // Lookup struct type in registry
                    let stype_val = match self.struct_types.find(struct_name.as_str()) { Some(v) => v, None => return self.report("Unknown struct type in literal") };
                    if stype_val.value_type != crate::value::ValueType::ValueObject { return self.report("Struct type registry entry invalid"); }
                    if unsafe { (*stype_val.value_as.object).obj_type } != ObjectType::ObjStructType { return self.report("Registry entry not struct type"); }
                    let stype_ptr = unsafe { stype_val.value_as.object as *mut crate::objects::object_struct::ObjectStructType };
                    // Validate fields: order doesn't need to match definition, we'll place by index.
                    let expected_count = unsafe { (*stype_ptr).field_names.len() };
                    if field_count != expected_count { return self.report("Field count mismatch in struct literal"); }
                    // Pop values in reverse order to collect, since stack has them in evaluation order.
                    let mut provided_values: Vec<(usize, Value)> = Vec::with_capacity(field_count);
                    for lname in literal_field_names.iter().rev() { // reverse to align with pop order
                        let val = self.pop();
                        // lookup index
                        let idx_val = unsafe { (*stype_ptr).field_index.find(lname.as_str()) };
                        if idx_val.is_none() { return self.report("Unknown field in struct literal"); }
                        let idx_num = idx_val.unwrap();
                        if !is_number(&idx_num) { return self.report("Corrupt field index table"); }
                        let slot = as_number(&idx_num) as usize;
                        provided_values.push((slot, val));
                    }
                    provided_values.reverse();
                    // Allocate instance
                    let (inst_ptr, size) = self.object_manager.alloc_struct_instance(stype_ptr, expected_count);
                    for (slot, val) in provided_values.into_iter() { unsafe { (*inst_ptr).fields[slot] = val; } }
                    self.track_allocation(size);
                    // push instance value
                    self.push(Value { value_type: crate::value::ValueType::ValueObject, value_as: crate::value::ValueUnion { object: inst_ptr as *mut crate::objects::object::Object } });
                }
                Some(chunk::OpCode::StructInstantiateStack) => {
                    // Same layout as heap instantiate but produce stack struct
                    let type_name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiateStack (missing type name index)") } as usize;
                    let field_count = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiateStack (missing field count)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let type_name_value = unsafe { *(*chunk_ptr).get_constant(type_name_index) };
                    if !is_string(&type_name_value) { return self.report("StructInstantiateStack type name constant not string"); }
                    let mut literal_field_names: Vec<String> = Vec::with_capacity(field_count);
                    for _ in 0..field_count {
                        let fi = match self.read_byte() { Some(b) => b, None => return self.report("Malformed StructInstantiateStack (missing field name const index)") } as usize;
                        let fv = unsafe { *(*chunk_ptr).get_constant(fi) };
                        if !is_string(&fv) { return self.report("StructInstantiateStack field name constant not string"); }
                        let fname = unsafe { (*as_string_object(&fv)).content.clone() };
                        literal_field_names.push(fname);
                    }
                    let struct_name = unsafe { (*as_string_object(&type_name_value)).content.clone() };
                    let stype_val = match self.struct_types.find(struct_name.as_str()) { Some(v) => v, None => return self.report("Unknown struct type in stack literal") };
                    if stype_val.value_type != crate::value::ValueType::ValueObject { return self.report("Struct type registry entry invalid"); }
                    if unsafe { (*stype_val.value_as.object).obj_type } != ObjectType::ObjStructType { return self.report("Registry entry not struct type"); }
                    let stype_ptr = unsafe { stype_val.value_as.object as *mut ObjectStructType };
                    let expected_count = unsafe { (*stype_ptr).field_names.len() };
                    if field_count != expected_count { return self.report("Field count mismatch in stack struct literal"); }
                    let mut provided_values: Vec<(usize, Value)> = Vec::with_capacity(field_count);
                    for lname in literal_field_names.iter().rev() {
                        let val = self.pop();
                        let idx_val = unsafe { (*stype_ptr).field_index.find(lname.as_str()) };
                        if idx_val.is_none() { return self.report("Unknown field in stack struct literal"); }
                        let idx_num = idx_val.unwrap();
                        if !is_number(&idx_num) { return self.report("Corrupt field index table"); }
                        let slot = as_number(&idx_num) as usize;
                        provided_values.push((slot, val));
                    }
                    provided_values.reverse();
                    let mut fields = vec![Value::new(); expected_count];
                    for (slot, val) in provided_values.into_iter() { fields[slot] = val; }
                    if let Some(last) = self.frame_stack_structs.last_mut() {
                        last.push(StackStruct { struct_type: stype_ptr, fields });
                        let index = last.len() - 1;
                        self.push(Value { value_type: crate::value::ValueType::ValueStackStruct, value_as: crate::value::ValueUnion { stack_index: index } });
                    } else {
                        return self.report("No frame arena for stack struct");
                    }
                }
                Some(chunk::OpCode::GetField) => {
                    // Layout: GetField <field_name_const_index>
                    let field_name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed GetField (missing name index)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let name_val = unsafe { *(*chunk_ptr).get_constant(field_name_index) };
                    if !is_string(&name_val) { return self.report("GetField constant not string"); }
                    let field_name = unsafe { (*as_string_object(&name_val)).content.clone() };
                    let receiver = self.pop();
                    let value = match receiver.value_type {
                        crate::value::ValueType::ValueObject => {
                            let obj_ptr = unsafe { receiver.value_as.object };
                            let obj = unsafe { &*obj_ptr };
                            if obj.obj_type != ObjectType::ObjStructInstance { return self.report("Receiver not struct instance"); }
                            let inst_ptr = obj_ptr as *mut ObjectStructInstance;
                            let stype_ptr = unsafe { (*inst_ptr).struct_type };
                            let idx_val = unsafe { (*stype_ptr).field_index.find(field_name.as_str()) };
                            if idx_val.is_none() { return self.report("Unknown field on struct instance"); }
                            let idx_v = idx_val.unwrap(); if !is_number(&idx_v) { return self.report("Corrupt field index table"); }
                            let slot = as_number(&idx_v) as usize;
                            unsafe { (*inst_ptr).fields[slot] }
                        }
                        crate::value::ValueType::ValueStackStruct => {
                            let idx = unsafe { receiver.value_as.stack_index };
                            let arena = match self.frame_stack_structs.last() { Some(a) => a, None => return self.report("Missing frame arena") };
                            if idx >= arena.len() { return self.report("Invalid stack struct index"); }
                            let s = &arena[idx];
                            let idx_val = unsafe { (*s.struct_type).field_index.find(field_name.as_str()) };
                            if idx_val.is_none() { return self.report("Unknown field on stack struct") };
                            let idx_v = idx_val.unwrap(); if !is_number(&idx_v) { return self.report("Corrupt field index table"); }
                            let slot = as_number(&idx_v) as usize;
                            s.fields[slot]
                        }
                        _ => return self.report("Only instances have fields"),
                    };
                    self.push(value);
                }
                Some(chunk::OpCode::SetField) => {
                    // Layout: SetField <field_name_const_index>; stack: receiver value (value on top)
                    let field_name_index = match self.read_byte() { Some(b) => b, None => return self.report("Malformed SetField (missing name index)") } as usize;
                    let chunk_ptr = unsafe { self.current_chunk() } as *mut Box<Chunk>;
                    let name_val = unsafe { *(*chunk_ptr).get_constant(field_name_index) };
                    if !is_string(&name_val) { return self.report("SetField constant not string"); }
                    let field_name = unsafe { (*as_string_object(&name_val)).content.clone() };
                    let value = self.pop();
                    let receiver = self.pop();
                    match receiver.value_type {
                        crate::value::ValueType::ValueObject => {
                            let obj_ptr = unsafe { receiver.value_as.object };
                            let obj = unsafe { &*obj_ptr };
                            if obj.obj_type != ObjectType::ObjStructInstance { return self.report("Receiver not struct instance"); }
                            let inst_ptr = obj_ptr as *mut ObjectStructInstance;
                            let stype_ptr = unsafe { (*inst_ptr).struct_type };
                            let idx_val = unsafe { (*stype_ptr).field_index.find(field_name.as_str()) };
                            if idx_val.is_none() { return self.report("Unknown field on struct instance"); }
                            let idx_v = idx_val.unwrap(); if !is_number(&idx_v) { return self.report("Corrupt field index table"); }
                            let slot = as_number(&idx_v) as usize;
                            unsafe { (*inst_ptr).fields[slot] = value; }
                        }
                        crate::value::ValueType::ValueStackStruct => {
                            let idx = unsafe { receiver.value_as.stack_index };
                            let arena = match self.frame_stack_structs.last_mut() { Some(a) => a, None => return self.report("Missing frame arena") };
                            if idx >= arena.len() { return self.report("Invalid stack struct index"); }
                            let s = &mut arena[idx];
                            let idx_val = unsafe { (*s.struct_type).field_index.find(field_name.as_str()) };
                            if idx_val.is_none() { return self.report("Unknown field on stack struct"); }
                            let idx_v = idx_val.unwrap(); if !is_number(&idx_v) { return self.report("Corrupt field index table"); }
                            let slot = as_number(&idx_v) as usize;
                            s.fields[slot] = value;
                        }
                        _ => return self.report("Only instances have fields"),
                    }
                    // push assigned value like typical expression semantics
                    self.push(value);
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
        // Root ordering: add new upvalue pointer to open_upvalues (a GC root set) BEFORE tracking bytes.
        let (new_up, size) = self.object_manager.alloc_upvalue(slot_ptr);
        self.open_upvalues.push(new_up);
        self.track_allocation(size);
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
       // Index-based loop to permit mutable borrows inside
       for i in 0..self.open_upvalues.len() {
           let up_ptr = self.open_upvalues[i];
           let loc = unsafe { (*up_ptr).location };
           if loc >= last_ptr {
               // Copy value then possibly promote
               let mut v = unsafe { *loc };
               v = self.promote_stack_struct_value_reason(v, Some("closure capture"), 0);
               unsafe {
                   (*up_ptr).closed = v;
                   (*up_ptr).location = &mut (*up_ptr).closed as *mut Value;
               }
           }
       }
    }

    // Promote a ValueStackStruct to a heap ObjectStructInstance (deeply promoting nested stack structs)
    fn promote_stack_struct_value_reason(&mut self, value: Value, reason: Option<&str>, depth: usize) -> Value {
        if value.value_type != crate::value::ValueType::ValueStackStruct { return value; }
        if depth == 0 {
            if let Some(r) = reason { self.warn(&format!("Implicit promotion of stack struct to heap ({})", r)); }
        }
        let idx = unsafe { value.value_as.stack_index };
        // Extract metadata and a raw pointer to fields without holding an immutable borrow across allocations.
        let (struct_type_ptr, field_len, fields_ptr) = {
            match self.frame_stack_structs.last() {
                Some(arena) => {
                    if idx >= arena.len() { return value; }
                    let s = &arena[idx];
                    (s.struct_type, s.fields.len(), s.fields.as_ptr())
                }
                None => return value,
            }
        };
        // Allocate heap instance
        let (inst_ptr, size) = self.object_manager.alloc_struct_instance(struct_type_ptr, field_len);
        // Copy and promote each field without cloning the entire vector
        for i in 0..field_len {
            let fv = unsafe { *fields_ptr.add(i) };
            unsafe { (*inst_ptr).fields[i] = self.promote_stack_struct_value_reason(fv, None, depth + 1); }
        }
        self.track_allocation(size);
        Value { value_type: crate::value::ValueType::ValueObject, value_as: crate::value::ValueUnion { object: inst_ptr as *mut crate::objects::object::Object } }
    }

    fn report(&mut self, message: &str) -> Result<InterpretResult, String> {
        self.report_runtime_error(message)
    }

    fn report_runtime_error(&mut self, message: &str) -> Result<InterpretResult, String> {
        self.runtime_error(message)
    }

    fn runtime_error(&mut self, message: &str) -> Result<InterpretResult, String> {
    // Calculate instruction offset for error reporting
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
    assert!(vm.interpret(r"!(5 - 4 > 3 * 2 == !nil);") == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_string_concatenate() {
        let mut vm = VM::new();
    assert!(vm.interpret(r#""st" + "ri" + "ng";"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_statement() {
        let mut vm = VM::new();
    assert!(vm.interpret(r#"print 1 + 2; print 3 * 4;"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_global_var() {
        let mut vm = VM::new();
    assert!(vm.interpret(r#"var beverage = "coffee"; 
                var breakfast = "beignets with " + beverage;
                print breakfast;"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_define_global_var() {
        let mut vm = VM::new();
    assert!(vm.interpret(r#"var beverage = "coffee";"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_print_local_var() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"{var a = "hello world!"; a = "111"; print a;}"#) == InterpretResult::InterpretOk);
        assert!(vm.interpret(r#"{
                                var a = "the first";
                                {
                                    var a = "the second";
                                    print a;
                                }
                                print a;
                            }"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_if_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"print "test if statement...";
                            if (1 > 0) {
                                print "'1 > 0' is true";
                            }"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_else_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"print "test else clause...";
                            if (1 < 0) {
                                print "'1 < 0' is true";
                            } else {
                                print "'1 < 0' is false";
                            }"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_and_operator() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"print "test and operator...";
                            if ( 1 > 0 and 2 > 1) {
                                print "'1 > 0 and 2 > 1' is true";
                            } else {
                                print "'1 > 0 and 2 > 1' is false";
                            }
                            
                            if ( 1 > 0 and 2 < 1) {
                                print "'1 > 0 and 2 < 1' is true";
                            } else {
                                print "'1 > 0 and 2 < 1' is false";
                            }"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_or_operator() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"print "test or operator...";
                            if ( 1 > 0 or 2 > 1) {
                                print "'1 > 0 or 2 > 1' is true";
                            } else {
                                print "'1 > 0 or 2 > 1' is false";
                            }
                            
                            if ( 1 > 0 or 2 < 1) {
                                print "'1 > 0 or 2 < 1' is true";
                            } else {
                                print "'1 > 0 or 2 < 1' is false";
                            }"#) == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_while_statement() {
        let mut vm = VM::new();
        assert!(vm.interpret(r#"print "test while statement...";
                            var count = 1;
                            while (count > 0) {
                                print count;
                                count = count - 1;
                            }"#) == InterpretResult::InterpretOk);
    }
 
    #[test]
    fn test_for_statement() {
        let mut vm = VM::new();
        let result = vm.interpret(r#"print "test for statement...";
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
                            }"#);
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_function_call() {
        let mut vm = VM::new();
        let result = vm.interpret(
            r#"fn sum(a, b, c) {
                        return a + b + c;
                    }
                    print 4 + sum(5, 6, 7);"#);
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_native_function_call() {
        let mut vm = VM::new();
        let result = vm.interpret(
            r#"print clock();"#);
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_fib_function() {
        let mut vm = VM::new();
        let result = vm.interpret(r#"
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
            print end - start;"#);
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_closure() {
        let mut vm = VM::new();
        let result = vm.interpret(r#"
            fn outer() {
                var x = "outside";
                fn inner() {
                    print x;
                }
                return inner;
            }
            var closure = outer();
            closure();"#);
        assert!(result == InterpretResult::InterpretOk);
    }

    #[test]
    fn test_closure_with_shared_variable() {
        let mut vm = VM::new();
        let result = vm.interpret(r#"
            var globalSet;
            var globalGet;

            fn main() {
                var a = "initial";

                fn set(value) { a = value; }
                fn get() { print a; }

                globalSet = set;
                globalGet = get;
            }

            main();
            globalSet("updated");
            globalGet();
            globalSet("initial");
            globalGet();"#);
        assert!(result == InterpretResult::InterpretOk);
    }    

    #[test]
    fn test_gc_pressure_many_strings() {
        let mut vm = VM::new();
        // Force an early GC so we can observe at least one cycle during this test without huge allocations.
        vm.set_gc_threshold(0);
        // Builds increasingly large string causing many intermediate unreachable strings.
        let script = r#"
            var s = "";
            var i = 0;
            while (i < 1500) {
                s = s + "abcdefgh";
                i = i + 1;
            }"#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
        // Ensure at least one GC cycle ran under allocation pressure.
        assert!(vm.gc.stats().cycles > 0, "Expected GC cycles > 0, got {}", vm.gc.stats().cycles);
    }

    #[test]
    fn test_gc_pressure_functions_and_closures_original() {
        // Original failing pattern: function defined inside loop then immediately called.
        let mut vm = VM::new();
        vm.set_gc_threshold(0);
        // Restored higher iteration count to increase allocation pressure & exercise multiple GC cycles.
        let script = r#"
            var i = 0;
            while (i < 300) {
                fn f() {
                    return i;
                }
                f();
                i = i + 1;
            }"#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
        assert!(vm.gc.stats().cycles > 0, "Expected GC cycles > 0, got {}", vm.gc.stats().cycles);
    }

    #[test]
    fn test_trait_impl_parsing_only() {
        let mut vm = VM::new();
        let script = r#"
            trait Printable {
                fn print_self();
                fn clone();
            }

            impl Printable for string {
                fn print_self() { print "impl running"; }
                fn clone() { print "clone"; }
            }

            print "after trait/impl";
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
    }

    #[test]
    fn test_struct_declaration_parsing_only() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y, }
            print "struct parsed";
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
    }

    #[test]
    fn test_struct_literal_basic() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = Point { x = 1, y = 2 };
            print p.x; // expect 1
            print p.y; // expect 2
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
    }

    #[test]
    fn test_struct_literal_field_order_swap() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = Point { y = 5, x = 3 }; // order reversed
            print p.x; // expect 3
            print p.y; // expect 5
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
    }

    #[test]
    fn test_struct_field_assignment() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = Point { x = 10, y = 20 };
            p.x = 42;
            print p.x; // expect 42
            p.y = p.x + 1; // 43
            print p.y; // expect 43
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretOk);
    }

    #[test]
    fn test_struct_literal_missing_field_error() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = Point { x = 1 }; // missing y
        "#;
        let result = vm.interpret(script);
        assert_eq!(result, InterpretResult::InterpretRuntimeError);
    }

    #[test]
    fn test_struct_field_wrong_receiver_errors() {
        let mut vm = VM::new();
        let script_get = r#"
            var a = 1; a.value; // invalid get
        "#;
        assert_eq!(vm.interpret(script_get), InterpretResult::InterpretRuntimeError);

        let mut vm2 = VM::new();
        let script_set = r#"
            var a = 1; a.value = 2; // invalid set
        "#;
        assert_eq!(vm2.interpret(script_set), InterpretResult::InterpretRuntimeError);
    }

    #[test]
    fn test_new_struct_literal_basic() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = new Point { x = 7, y = 9 };
            print p.x; // 7
            print p.y; // 9
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_new_struct_field_assignment() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = new Point { x = 0, y = 0 };
            p.x = 11;
            p.y = p.x + 1; // 12
            print p.x; print p.y;
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_return_stack_struct_compile_error() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            fn make() { return Point { x = 1, y = 2 }; }
        "#; // returning stack struct literal should be compile error
        assert_eq!(vm.interpret(script), InterpretResult::InterpretCompileError);
    }

    #[test]
    fn test_return_heap_struct_allowed() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            fn make() { return new Point { x = 1, y = 2 }; }
            var p = make();
            print p.x; print p.y;
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_closure_captures_promoted_struct() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            fn makeGetter() {
                var p = Point { x = 3, y = 4 };
                fn getX() { return p.x; }
                return getX;
            }
            var gx = makeGetter();
            print gx(); // expect 3
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_global_promotion_struct() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var gp = Point { x = 10, y = 20 }; // should promote to heap
            gp.x = 15;
            print gp.x; // expect 15
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_return_expression() {
        let mut vm = VM::new();
        let script = r#"
            fn f() { return 1 + 2; }
            print f(); // expect 3
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_nested_struct_promotion_in_global() {
        let mut vm = VM::new();
        let script = r#"
            struct Inner { a }
            struct Outer { i }
            var g = Outer { i = Inner { a = 7 } }; // promotion should deep-copy inner
            print g.i.a; // expect 7
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_nested_struct_promotion_in_closure() {
        let mut vm = VM::new();
        let script = r#"
            struct Inner { a }
            struct Outer { i }
            fn make() {
                var o = Outer { i = Inner { a = 42 } };
                fn get() { return o.i.a; }
                return get;
            }
            var g = make();
            print g(); // expect 42
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_heap_instance_assignment_alias() {
        // Assigning a heap struct instance to another variable should alias the same object.
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = new Point { x = 1, y = 2 };
            var q = p; // alias, no deep copy
            q.x = 99;
            print p.x; // expect 99
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_heap_instance_pass_by_reference() {
        // Passing a heap struct instance to a function should mutate the same object.
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            fn setX(pt) { pt.x = 77; }
            var p = new Point { x = 1, y = 2 };
            setX(p);
            print p.x; // expect 77
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_stack_instance_assignment_alias_same_frame() {
        // Stack-allocated struct aliases by index within the same frame.
        let mut vm = VM::new();
        let script = r#"
            struct P { a }
            {
                var s = P { a = 5 };
                var t = s; // alias same stack struct within frame
                t.a = 8;
                print s.a; // expect 8
            }
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_trait_invoke_on_struct() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }

            trait Summable {
                fn sum();
                fn add(v);
            }

            impl Summable for Point {
                fn sum() { return self.x + self.y; }
                fn add(v) { return self.x + self.y + v; }
            }

            var p = new Point { x = 2, y = 3 };
            print p.sum(); // 5
            print p.add(5); // 10
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretOk);
    }

    #[test]
    fn test_invoke_unknown_method_errors() {
        let mut vm = VM::new();
        let script = r#"
            struct Point { x, y }
            var p = new Point { x = 1, y = 2 };
            p.nope(); // no impl registered
        "#;
        assert_eq!(vm.interpret(script), InterpretResult::InterpretRuntimeError);
    }
}