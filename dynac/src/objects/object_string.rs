use crate::objects::object::{Object, ObjectType};

#[repr(C)]
#[derive(Hash, Clone)]
pub struct ObjectString {
    pub object: Object,
    pub content: String,
}

impl ObjectString {
    pub fn new(content: &str) -> Self {
        ObjectString{
            object: Object {
                    obj_type: ObjectType::ObjString,
                },
            content: content.to_string()
        }
    }
}

impl PartialEq<ObjectString> for ObjectString {
    fn eq(&self, other: &ObjectString) -> bool {
        self.object == other.object && self.content == other.content
    }
}

impl Eq for ObjectString {
}

mod debug_feature {
    use crate::objects::object_string::ObjectString;

    impl Drop for ObjectString {
        fn drop(&mut self) {
            print!("drop string object: ");
            let object_string = std::ptr::from_mut(self) as *const ObjectString;
            println!("type=ObjectString, content={}", unsafe {
                (*object_string).content.as_str()
            });
        }
    }
}