
use crate::{
    objects::object::{Object, ObjectType},
    value::{make_nil_value, print_value, Value},
};

#[derive(Clone)]
#[allow(dead_code)]
pub struct ObjectUpvalue {
    pub object: Object,
    pub location: *mut Value,
    pub closed: Value,
}

#[allow(dead_code)]
impl ObjectUpvalue {
    pub fn new(location: *mut Value) -> Self {
        ObjectUpvalue {
            object: Object {
                obj_type: ObjectType::ObjUpvalue,
            },
            location,
            closed: make_nil_value(),
        }
    }

    /// Returns the raw pointer to the upvalue's location.
    pub fn location(&self) -> *mut Value {
        self.location
    }

    pub fn print(&self) {
        print!(
            "ObjectUpvalue: ObjectType={:?}, location=",
            self.object.obj_type
        );
        match unsafe { self.location.as_ref() } {
            Some(val) => print_value(val),
            None => print!("<null>"),
        }
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
    // use crate::objects::object_upvalue::ObjectUpvalue;

    // impl Drop for ObjectUpvalue {
    //     fn drop(&mut self) {
    //         print!("drop upvalue object: ");
    //         //let object_closure = std::ptr::from_mut(self) as *const ObjectUpvalue;
    //         self.print();
    //         //println!("type=ObjectUpvalue");
    //     }
    // }
}