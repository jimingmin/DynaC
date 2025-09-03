use crate::objects::object::{Object, ObjectType};

#[repr(C)]
#[derive(Clone)]
pub struct ObjectTrait {
    pub object: Object,
    pub name: String,
    pub method_names: Vec<String>, // signatures tracked later
}

impl ObjectTrait {
    pub fn new(name: String) -> Self {
        Self { object: Object { obj_type: ObjectType::ObjTrait }, name, method_names: Vec::new() }
    }
}
