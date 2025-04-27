#[derive(Debug, PartialEq)]
pub enum ObjectType {
    ObjString,
}
#[repr(C)]
pub struct Object {
    pub obj_type: ObjectType,
    pub next: *mut Object,
}
#[repr(C)]
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