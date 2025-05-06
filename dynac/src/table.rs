use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{self, Rc};

use crate::object::{self, ObjectString};
use crate::value::Value;


pub struct Table {
    //entries: HashMap<Rc<str>, Rc<ObjectString>>,
    entries: HashMap<Rc<str>, *const ObjectString>
}


impl Table {
    // pub fn new() -> Rc<RefCell<Self>> {
    //     Rc::new(RefCell::new(Table { entries: HashMap::new() }))
    // }

    pub fn new() -> Box<Table> {
        Box::new(Table { entries: HashMap::new() })
    }

    pub fn insert(&mut self, object_string: *const ObjectString) {
        let key = Rc::from((unsafe { &*object_string }).content.as_str());
        self.entries.insert(key, object_string);
    }

    pub fn find(&self, key: &str) -> Option<*const ObjectString>{
        self.entries.get(key).copied()
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