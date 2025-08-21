use std::ptr::NonNull;
use crate::{constants::MAX_STACK_SIZE, objects::{object::{Object, ObjectType}, object_closure::ObjectClosure, object_function::ObjectFunction}, value::{Value}};

pub struct CallFrame {
    callalbe_object: *mut Object,
    ip: usize,
    stack_base: NonNull<Value>,
    stack_base_offset: usize,
    stack_top_pos: usize,
}

impl CallFrame {
    pub fn new(stack_base: NonNull<Value>, stack_base_offset: usize) -> Self {
        CallFrame {
            callalbe_object: std::ptr::null_mut(),
            ip: 0,
            stack_base,
            stack_base_offset,
            stack_top_pos: 0
        }
    }

    #[inline(always)]
    pub fn set_callable_object(&mut self, object: *mut Object) {
        //ObjectFunction::new(0, String::new());
        //let fun = Rc::new(RefCell::new(ObjectFunction::new(0, String::new())));
        self.callalbe_object = object
    }

    #[inline(always)]
    pub fn function(&mut self) -> &mut ObjectFunction {
        assert!((unsafe { &*self.callalbe_object} ).obj_type == ObjectType::ObjFunction);
        unsafe { &mut *(self.callalbe_object as *mut ObjectFunction) }
    }

    #[inline(always)]
    pub fn closure(&mut self) -> &mut ObjectClosure {
        assert!((unsafe { &*self.callalbe_object} ).obj_type == ObjectType::ObjClosure);
        unsafe { &mut *(self.callalbe_object as *mut ObjectClosure) }
    }

    pub fn object_type(&self) -> ObjectType {
        (unsafe { &*self.callalbe_object} ).obj_type.clone()
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
        assert!(self.stack_top_pos + offset < MAX_STACK_SIZE);
        unsafe {
            &*self.stack_base.as_ptr().add(offset)
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