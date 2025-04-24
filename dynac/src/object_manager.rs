use crate::{object::{self, Object}, value::{as_mutable_object, is_object, Value}};


pub struct ObjectManager {
    size: usize,
    objects: *mut Object,
}

impl ObjectManager {
    pub fn new() -> Box<Self> {
        Box::new(ObjectManager{
            size: 0,
            objects: std::ptr::null_mut()
        })
    }

    pub fn push_object(&mut self, object: *mut Object) {
        if !self.objects.is_null() {
            unsafe {
                (*object).next = self.objects;
            }
            self.size += 1;
        }
        self.objects = object;
    }

    pub fn push_object_value(&mut self, value: &mut Value) {
        if is_object(value) {
            self.push_object(as_mutable_object(value));
        }
    }

    pub fn pop_object(&mut self) -> *mut Object {
        if self.objects.is_null() {
            return std::ptr::null_mut();
        }

        let object = self.objects;
        unsafe {
            self.objects = (*self.objects).next;
        }
        self.size -= 1;
        object
    }
}