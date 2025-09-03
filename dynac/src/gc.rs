use std::collections::HashSet;
use crate::{
    objects::{
        object::{Object, ObjectType},
        object_manager::ObjectManager,
        object_upvalue::ObjectUpvalue,
    },
    value::{Value, is_object, as_object},
    table::Table,
    call_frame::CallFrame,
};

pub struct GarbageCollector {
    white_set: HashSet<*mut Object>,
    gray_set: HashSet<*mut Object>,
    black_set: HashSet<*mut Object>,
    stats: GCStats,
}

/// Aggregated GC statistics (does not include currently-live total bytes; VM tracks that).
#[derive(Default, Debug, Clone)]
pub struct GCStats {
    pub cycles: u64,
    pub total_freed_bytes: usize,
    pub last_freed_bytes: usize,
    pub last_before_bytes: usize,
    pub last_after_bytes: usize,
    pub last_next_trigger_bytes: usize,
}

impl GCStats {
    fn record(&mut self, before: usize, freed: usize, after: usize, next_trigger: usize) {
        self.cycles += 1;
        self.total_freed_bytes += freed;
        self.last_freed_bytes = freed;
        self.last_before_bytes = before;
        self.last_after_bytes = after;
        self.last_next_trigger_bytes = next_trigger;
    }
}

// Lightweight tracing macro (only active with gc_debug feature)
#[cfg(feature = "gc_debug")]
macro_rules! gc_trace { ($($arg:tt)*) => { eprintln!("[gc-trace] {}", format_args!($($arg)*)); } }
#[cfg(not(feature = "gc_debug"))]
macro_rules! gc_trace { ($($arg:tt)*) => { } }
pub(crate) use gc_trace; // re-export for potential external module use

impl GarbageCollector {
    pub fn new() -> Self {
        Self {
            white_set: HashSet::new(),
            gray_set: HashSet::new(),
            black_set: HashSet::new(),
            stats: GCStats::default(),
        }
    }

    // Initialize the collector with all objects in white set
    pub fn prepare_collection(&mut self, object_manager: &ObjectManager) {
        self.reset();
        // Add all objects to white set initially
        for &obj_ptr in object_manager.iter() {
            self.white_set.insert(obj_ptr);
        }
    }

    // Mark a single object as gray (moves from white to gray set)
    pub fn mark_object(&mut self, obj: *mut Object) {
        if obj.is_null() || self.black_set.contains(&obj) {
            return;
        }

        if self.white_set.remove(&obj) {
            self.gray_set.insert(obj);
            gc_trace!("mark_object enqueue gray ptr={:p}", obj);
        }
    }

    // Mark a value (if it's an object)
    pub fn mark_value(&mut self, value: &Value) {
        if !is_object(value) {
            return;
        }
        self.mark_object(as_object(value) as *mut Object);
    }

    // Process gray objects until none remain
    pub fn trace_references(&mut self) {
        while !self.gray_set.is_empty() {
            let obj = *self.gray_set.iter().next().unwrap();
            self.gray_set.remove(&obj);
            self.black_set.insert(obj);
            
            unsafe {
                gc_trace!("trace gray -> black ptr={:p}", obj);
                self.blacken_object(obj);
            }
        }
    }

    // Mark all references in an object
    unsafe fn blacken_object(&mut self, object: *mut Object) {
        match (*object).obj_type {
            ObjectType::ObjClosure => {
                let closure = (*object).as_closure();
                self.mark_object(closure.function as *mut Object);
                for upvalue in &closure.upvalues {
                    self.mark_object(*upvalue as *mut Object);
                }
            }
            ObjectType::ObjFunction => {
                let function = (*object).as_function();
                for constant in function.chunk.iter_constants() {
                    self.mark_value(constant);
                }
            }
            ObjectType::ObjUpvalue => {
                let upvalue = (*object).as_upvalue();
                self.mark_value(&*upvalue.location);
            }
            ObjectType::ObjStructType => {
                // Only owns strings already in intern table; name & field_names are plain Strings (no GC Values)
            }
            ObjectType::ObjStructInstance => {
                let inst = (*object).as_struct_instance();
                self.mark_object(inst.struct_type as *mut Object);
                for field in &inst.fields { self.mark_value(field); }
            }
            _ => {}
        }
    }

    // Sweep phase - returns the set of unreachable objects
    pub fn sweep(&mut self, object_manager: &mut ObjectManager) -> usize {
        let mut freed_bytes = 0;
        for &obj_ptr in self.white_set.iter() {
            unsafe {
                // Account for the object's deep size before freeing it
                freed_bytes += (*obj_ptr).deep_size();
            }
            object_manager.remove_object(obj_ptr);
            unsafe {
                drop(Box::from_raw(obj_ptr));
            }
        }
        self.white_set.clear();
        gc_trace!("sweep freed_bytes={}", freed_bytes);
        freed_bytes
    }

    // Reset collector state
    pub fn reset(&mut self) {
        self.white_set.clear();
        self.gray_set.clear();
        self.black_set.clear();
    }

    // Mark roots provided by the VM
    pub fn mark_roots(&mut self, 
        stack: &[Value], 
        stack_top: usize,
        globals: &Table,
        intern_strings: &Table,
        frames: &[Box<CallFrame>],
        open_upvalues: &[*mut ObjectUpvalue]) {
        
        // Mark stack values
        for value in &stack[0..stack_top] {
            self.mark_value(value);
        }

    // Mark globals and interned strings
    for (_, value) in globals.iter() { self.mark_value(value); }
    for (_, value) in intern_strings.iter() { self.mark_value(value); }

        // Mark callframes - we'll mark the stack values which contain
        // the function/closure objects, since they are also stored there
        for frame in frames {
            let base = frame.get_stack_base_offset();
            let value = &stack[base]; // The function/closure is always at base
            self.mark_value(value);
        }

        // Mark open upvalues
        for upvalue in open_upvalues {
            self.mark_object(*upvalue as *mut Object);
        }
    }

    /// Record a completed GC cycle (invoked by VM which knows bytes before/after & threshold)
    pub fn record_cycle(&mut self, before: usize, freed: usize, after: usize, next_trigger: usize) {
        self.stats.record(before, freed, after, next_trigger);
        gc_trace!("cycle summary cycles={} freed={} before={} after={} next_trigger={}", self.stats.cycles, freed, before, after, next_trigger);
    }

    pub fn stats(&self) -> &GCStats { &self.stats }
}

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{objects::object_manager::ObjectManager, table::Table, value::{Value, ValueType, ValueUnion}};

        fn value_from_object(ptr: *mut Object) -> Value {
            Value { value_type: ValueType::ValueObject, value_as: ValueUnion { object: ptr } }
        }

        #[test]
        fn gc_collects_unreachable_objects() {
            let mut manager = ObjectManager::new();
            // Roots: only keep first string
            let (keep, _) = manager.alloc_string("keep");
            let (_drop1, _) = manager.alloc_string("drop1");
            let (_drop2, _) = manager.alloc_string("drop2");
            let (_drop3, _) = manager.alloc_string("drop3");

            let mut gc = GarbageCollector::new();
            let mut stack = [Value::new(); 8];
            stack[0] = value_from_object(keep as *mut Object);
            let stack_top = 1;
            let globals = Table::new();
            let frames: Vec<Box<CallFrame>> = vec![];
            let open_upvalues: Vec<*mut ObjectUpvalue> = vec![];

            let intern_strings = Table::new();
            gc.prepare_collection(&manager);
            gc.mark_roots(&stack, stack_top, &globals, &intern_strings, &frames, &open_upvalues);
            gc.trace_references();
            let freed = gc.sweep(&mut manager);
            assert!(freed > 0, "Expected some bytes to be freed");
            let remaining = manager.iter().count();
            assert_eq!(remaining, 1, "Only the rooted object should remain (got {remaining})");
        }

        #[test]
        fn gc_preserves_reachable_closure_and_function() {
            let mut manager = ObjectManager::new();
            let (func_root, _) = manager.alloc_function(0, "f1".to_string());
            let (closure_root, _) = manager.alloc_closure(func_root);
            let (_func_unreachable, _) = manager.alloc_function(0, "f2".to_string());

            let mut gc = GarbageCollector::new();
            let mut stack = [Value::new(); 8];
            stack[0] = value_from_object(closure_root as *mut Object);
            let stack_top = 1;
            let globals = Table::new();
            let frames: Vec<Box<CallFrame>> = vec![];
            let open_upvalues: Vec<*mut ObjectUpvalue> = vec![];

            let intern_strings = Table::new();
            gc.prepare_collection(&manager);
            gc.mark_roots(&stack, stack_top, &globals, &intern_strings, &frames, &open_upvalues);
            gc.trace_references();
            gc.sweep(&mut manager);
            let remaining = manager.iter().count();
            assert_eq!(remaining, 2, "Closure and its function should remain");
        }

        #[test]
        fn gc_marks_via_upvalue() {
            let mut manager = ObjectManager::new();
            let (string_ptr, _) = manager.alloc_string("captured");
            let mut stack = [Value::new(); 8];
            stack[0] = value_from_object(string_ptr as *mut Object);
            let stack_top = 1; // one value on stack
            // Allocate upvalue pointing to stack[0]
            let (upvalue_ptr, _) = manager.alloc_upvalue(&mut stack[0] as *mut Value);

            let mut gc = GarbageCollector::new();
            let globals = Table::new();
            let frames: Vec<Box<CallFrame>> = vec![];
            let open_upvalues: Vec<*mut ObjectUpvalue> = vec![upvalue_ptr];

            let intern_strings = Table::new();
            gc.prepare_collection(&manager);
            gc.mark_roots(&stack, stack_top, &globals, &intern_strings, &frames, &open_upvalues);
            gc.trace_references();
            gc.sweep(&mut manager);
            let remaining = manager.iter().count();
            assert_eq!(remaining, 2, "Upvalue and captured string should remain");
        }

        #[test]
        fn gc_stats_record_cycle() {
            let mut gc = GarbageCollector::new();
            assert_eq!(gc.stats().cycles, 0);
            gc.record_cycle(1000, 400, 600, 1200);
            assert_eq!(gc.stats().cycles, 1);
            assert_eq!(gc.stats().last_before_bytes, 1000);
            assert_eq!(gc.stats().last_freed_bytes, 400);
            assert_eq!(gc.stats().last_after_bytes, 600);
            assert_eq!(gc.stats().last_next_trigger_bytes, 1200);
            assert_eq!(gc.stats().total_freed_bytes, 400);
        }
    }
