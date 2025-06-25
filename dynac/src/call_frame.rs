use std::{cell::{Ref, RefCell, RefMut, UnsafeCell}, ptr::NonNull, rc::Rc};
use std::sync::Once;
use crate::{constants::MAX_STACK_SIZE, objects::object_function::ObjectFunction, value::{self, Value}};

pub struct CallFrame {
    function: *mut ObjectFunction,
    ip: usize,
    stack_base: NonNull<Value>,
    stack_base_offset: usize,
    stack_top_pos: usize,
}

static mut SHARED_FUNCTION: Option<Rc<RefCell<ObjectFunction>>> = None;
static INIT: Once = Once::new();

fn get_shared_function() -> &'static Rc<RefCell<ObjectFunction>> {
    INIT.call_once(|| {
        unsafe {
            SHARED_FUNCTION = Some(Rc::new(RefCell::new(ObjectFunction::new(0, "".to_string()))));
        }
    });
    unsafe { SHARED_FUNCTION.as_ref().unwrap() }
}

impl CallFrame {
    pub fn new(stack_base: NonNull<Value>, stack_base_offset: usize) -> Self {
        CallFrame {
            function: std::ptr::null_mut(),
            ip: 0,
            stack_base,
            stack_base_offset,
            stack_top_pos: 0
        }
    }

    #[inline(always)]
    pub fn set_function(&mut self, function: *mut ObjectFunction) {
        //ObjectFunction::new(0, String::new());
        //let fun = Rc::new(RefCell::new(ObjectFunction::new(0, String::new())));
        self.function = function
    }

    #[inline(always)]
    pub fn function(&mut self) -> &mut ObjectFunction {
        unsafe { &mut *self.function }
    }

    #[inline(always)]
    pub fn get_stack_base(&mut self) -> &mut NonNull<Value> {
        &mut self.stack_base
    }

    #[inline(always)]
    pub fn get_stack_base_offset(&self) -> usize {
        self.stack_base_offset
    }

    #[inline(always)]
    pub fn get_stack_value(&self, offset: usize) -> &Value {
        assert!(self.stack_top_pos + offset + 1 < MAX_STACK_SIZE);
        unsafe {
            &*self.stack_base.as_ptr().add(offset + 1)
        }
    }

    #[inline(always)]
    pub fn set_stack_value(&mut self, offset: usize, value: Value) {
        assert!(self.stack_top_pos + offset < MAX_STACK_SIZE);
        unsafe {
            *self.stack_base.as_ptr().add(offset) = value;
        }
    }

    #[inline(always)]
    pub fn ip(&mut self) -> &mut usize {
        &mut self.ip
    }
}