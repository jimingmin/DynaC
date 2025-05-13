#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum ObjectType {
    ObjString,
}
#[repr(C)]
#[derive(Hash, Clone)]
pub struct Object {
    pub obj_type: ObjectType,
    pub next: *mut Object,
}
#[repr(C)]
#[derive(Hash, Clone)]
pub struct ObjectString {
    pub object: Object,
    pub content: String,
}

impl ObjectString {
    pub fn new(content: &str) -> Box<ObjectString> {
        Box::new(
            ObjectString{
                object: Object {
                        obj_type: ObjectType::ObjString,
                        next: std::ptr::null_mut(),
                    },
                content: content.to_string()
        })
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Object) -> bool {
        self.obj_type == other.obj_type
    }
}

impl Eq for Object {
}

impl PartialEq<ObjectString> for ObjectString {
    fn eq(&self, other: &ObjectString) -> bool {
        self.object == other.object && self.content == other.content
    }
}

impl Eq for ObjectString {
}

#[cfg(feature = "debug_trace_execution")]
mod debug_feature {
    use crate::object::ObjectType;

    use super::{Object, ObjectString};

    impl Drop for Object {
        fn drop(&mut self) {
            print!("drop object: ");
            match self.obj_type {
                ObjectType::ObjString => {
                    let object_string = std::ptr::from_mut(self) as *const ObjectString;
                    println!("type=ObjectString, content={}", unsafe {
                        (*object_string).content.as_str()
                    });
                }
            }
        }
    }
}

