use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{self, Rc};

use crate::object::{self, ObjectString};
use crate::value::{as_string_object, Value, ValueType};


pub struct Table {
    //entries: HashMap<Rc<str>, Rc<ObjectString>>,
    entries: HashMap<String, Value>
}


impl Table {
    // pub fn new() -> Rc<RefCell<Self>> {
    //     Rc::new(RefCell::new(Table { entries: HashMap::new() }))
    // }

    pub fn new() -> Box<Table> {
        Box::new(Table { entries: HashMap::new() })
    }

    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        //let key = Rc::from((unsafe { &*object_string }).content.as_str());
        if value.value_type == ValueType::ValueObject {
            let string = as_string_object(&value);
            //println!("insert key : {}, value : {}", key, unsafe {&(*string)}.content);
        }
        self.entries.insert(key, value)
    }

    pub fn find(&self, key: &str) -> Option<Value>{
        self.entries.get(key).copied()
    }

    pub fn remove(&mut self, key: &String) -> Option<Value> {
        self.entries.remove(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    // pub fn insert(&mut self, object_string: Rc<ObjectString>) {
    //     let key = Rc::from(object_string.content.as_str());
    //     self.entries.insert(key, object_string);
    // }

    // pub fn find(&self, key: &str) -> Option<Rc<ObjectString>>{
    //     self.entries.get(key).cloned()
    // }
}