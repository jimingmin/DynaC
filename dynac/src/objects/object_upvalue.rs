use std::ptr::NonNull;

use crate::{objects::object::{Object, ObjectType}, value::Value};


#[derive(Clone)]
pub struct ObjectUpvalue {
    pub object: Object,
    pub location: NonNull<Value>,
}


impl ObjectUpvalue {
    pub fn new(slot: NonNull<Value>) ->Self {
        ObjectUpvalue {
            object: Object {
                obj_type: ObjectType::ObjUpvalue,
            },
            location: slot,
        }
    }
}