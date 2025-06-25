use std::{cell::RefCell, rc::{Rc, Weak}};

use crate::{objects::object::{self, Object}, value::{as_mutable_object, is_object, Value}};


pub struct ObjectManager {
    //size: usize,
    objects: Vec<*mut Object>,
}

impl ObjectManager {
    pub fn new() -> Self {
        ObjectManager{
            //size: 0,
            objects: vec![],//std::ptr::null_mut()
        }
    }

    pub fn push_object(&mut self, object: *mut Object) {
        if self.objects.contains(&object) {
            return;
        }

        self.objects.push(object);
        // if !self.objects.is_null() {
        //     unsafe {
        //         (*object).next = self.objects;
        //     }
        // }
        // self.size += 1;
        // self.objects = object;
    }

    pub fn push_object_value(&mut self, value: &mut Value) {
        if is_object(value) {
            self.push_object(as_mutable_object(value));
        }
    }

    pub fn pop_object(&mut self) -> *mut Object {
        if self.objects.is_empty() {
            return std::ptr::null_mut();
        }

        self.objects.pop().unwrap()
        // let object = self.objects;
        // unsafe {
        //     if !(*object).next.is_null() {
        //         self.objects = (*self.objects).next;
        //     } else {
        //         self.objects = std::ptr::null_mut();
        //     }
        // }
        // self.size -= 1;
        // object
    }
}