use crate::{chunk::Chunk, objects::object::{Object, ObjectType}};

#[repr(C)]
pub struct ObjectFunction {
    pub object: Object,
    pub arity: u8,
    pub chunk: Box<Chunk>,
    pub name: String,
}

impl ObjectFunction {
    pub fn new(arity: u8, name: String) -> Self {
        ObjectFunction {
                object: Object {
                    obj_type: ObjectType::ObjFunction,
                }, arity, chunk: Box::new(Chunk::new()), name}
    }

    pub fn chunk(&self) -> &Box<Chunk> {
        &self.chunk
    }
}


mod debug_feature {
    use crate::objects::object_function::ObjectFunction;

    impl Drop for ObjectFunction {
        fn drop(&mut self) {
            print!("drop function object: ");
            let object_function = std::ptr::from_mut(self) as *const ObjectFunction;
            println!("type=ObjectFunction, name={}", unsafe {
                (*object_function).name.as_str()
            });
        }
    }
}
