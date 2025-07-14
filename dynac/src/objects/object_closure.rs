use crate::objects::{object::{Object, ObjectType}, object_function::ObjectFunction, object_upvalue::ObjectUpvalue};


#[repr(C)]
pub struct ObjectClosure {
    pub object: Object,
    pub function: Box<ObjectFunction>,
    pub upvalues: Vec<ObjectUpvalue>,
}

impl ObjectClosure {
    pub fn new(function: Box<ObjectFunction>) -> Self {
        ObjectClosure {
            object: Object {
                obj_type: ObjectType::ObjClosure,
            },
            function,
            upvalues: vec![],
        }
    }
}

mod debug_feature {
    use crate::objects::object_closure::ObjectClosure;

    impl Drop for ObjectClosure {
        fn drop(&mut self) {
            print!("drop closure object: ");
            let object_closure = std::ptr::from_mut(self) as *const ObjectClosure;
            println!("type=ObjectClosure");
        }
    }
}