use crate::objects::object::{Object, ObjectType};
use crate::objects::object_function::ObjectFunction;

/// A closure object: holds a pointer to the function and indices of upvalues.
/// `upvalues` uses `Vec<usize>` (e.g. indices into VM.open_upvalues or another upvalue table)
/// as requested.
#[repr(C)]
#[derive(Clone)]
pub struct ObjectClosure {
    pub object: Object,
    pub function: *mut ObjectFunction,
    pub upvalues: Vec<usize>,
}

#[allow(dead_code)]
impl ObjectClosure {
    pub fn new(function: *mut ObjectFunction) -> Self {
        ObjectClosure {
            object: Object {
                obj_type: ObjectType::ObjClosure,
            },
            function,
            upvalues: Vec::new(),
        }
    }

    /// Add an upvalue index to this closure.
    pub fn add_upvalue(&mut self, index: usize) {
        self.upvalues.push(index);
    }

    /// Get a slice of upvalue indices.
    pub fn upvalues(&self) -> &[usize] {
        &self.upvalues
    }

    /// Convenience debug print.
    pub fn print(&self) {
        println!(
            "ObjectClosure: ObjectType={:?}, function={:?}, upvalues={:?}",
            self.object.obj_type, self.function, self.upvalues
        );
    }
}

mod debug_feature {
    use crate::objects::object_closure::ObjectClosure;

    impl Drop for ObjectClosure {
        fn drop(&mut self) {
            print!("drop closure object: ");
            let _object_closure = std::ptr::from_mut(self) as *const ObjectClosure;
            println!("type=ObjectClosure");
        }
    }
}