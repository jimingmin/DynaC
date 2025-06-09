use crate::objects::object::{Object, ObjectType};

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

impl PartialEq<ObjectString> for ObjectString {
    fn eq(&self, other: &ObjectString) -> bool {
        self.object == other.object && self.content == other.content
    }
}

impl Eq for ObjectString {
}