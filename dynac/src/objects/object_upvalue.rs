use std::ptr::NonNull;

use crate::{objects::object::{Object, ObjectType}, value::{make_nil_value, print_value, Value}};


#[derive(Clone, Copy)]
pub struct ObjectUpvalue {
    pub object: Object,
    pub location: NonNull<Value>,
    pub closed: Value,
}


impl ObjectUpvalue {
    pub fn new(slot: NonNull<Value>) ->Self {
        ObjectUpvalue {
            object: Object {
                obj_type: ObjectType::ObjUpvalue,
            },
            location: slot,
            closed: make_nil_value(),
        }
    }

    pub fn location(&self) -> &NonNull<Value> {
        &self.location
    }

    pub fn print(&self) {
        print!("ObjectUpvalue: ObjectType={:?}, location=", self.object.obj_type);
        print_value(unsafe { self.location.as_ref() });
        print!(", closed=");
        print_value(&self.closed);
        println!();
    }
}

// impl Clone for ObjectUpvalue {
//     fn clone(&self) -> Self {
//         Self { object: self.object.clone(), location: self.location.clone(), closed: self.closed.clone() }
//     }
// }

mod debug_feature {
    use crate::objects::object_upvalue::ObjectUpvalue;

    // impl Drop for ObjectUpvalue {
    //     fn drop(&mut self) {
    //         print!("drop upvalue object: ");
    //         //let object_closure = std::ptr::from_mut(self) as *const ObjectUpvalue;
    //         //self.print();
    //         println!("type=ObjectUpvalue");
    //     }
    // }
}