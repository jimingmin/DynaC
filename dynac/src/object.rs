#[derive(Debug, Hash, PartialEq, Eq)]
pub enum ObjectType {
    ObjString,
}
#[repr(C)]
#[derive(Hash)]
pub struct Object {
    pub obj_type: ObjectType,
    pub next: *mut Object,
}
#[repr(C)]
#[derive(Hash)]
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
