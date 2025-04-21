#[derive(Debug, PartialEq)]
pub enum ObjectType {
    ObjString,
}

pub struct Object {
    pub obj_type: ObjectType,
}

pub struct ObjectString {
    pub object: Object,
    pub content: String,
}

impl ObjectString {
    pub fn new(content: &str) -> Box<ObjectString> {
        Box::new(
            ObjectString{
                object: Object{
                        obj_type:ObjectType::ObjString
                    },
                content: content.to_string()
        })
    }
}