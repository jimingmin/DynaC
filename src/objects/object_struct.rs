use crate::{objects::object::{Object, ObjectType}, table::Table, value::Value};

#[repr(C)]
//#[derive(Clone)]
pub struct ObjectStructType {
    pub object: Object,
    pub name: String,
    pub field_names: Vec<String>, // index = field slot
    pub field_index: Table,        // name -> numeric Value index
}

impl ObjectStructType {
    pub fn new(name: String) -> Self {
        Self { object: Object { obj_type: ObjectType::ObjStructType }, name, field_names: Vec::new(), field_index: Table::new() }
    }
}

#[repr(C)]
//#[derive(Clone)]
pub struct ObjectStructInstance {
    pub object: Object,
    pub struct_type: *mut ObjectStructType,
    pub fields: Vec<Value>, // parallel to struct_type.field_names
}

impl ObjectStructInstance {
    pub fn new(struct_type: *mut ObjectStructType, field_count: usize) -> Self {
        Self { object: Object { obj_type: ObjectType::ObjStructInstance }, struct_type, fields: vec![Value::new(); field_count] }
    }
}