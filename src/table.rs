use std::collections::HashMap;
use crate::value::Value;


pub struct Table {
    //entries: HashMap<Rc<str>, Rc<ObjectString>>,
    entries: HashMap<String, Value>
}


#[allow(dead_code)]
impl Table {
    // pub fn new() -> Rc<RefCell<Self>> {
    //     Rc::new(RefCell::new(Table { entries: HashMap::new() }))
    // }

    pub fn new() -> Self {
        Table { entries: HashMap::new() }
    }

    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
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

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter()
    }
}