#[repr(C)]
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum ObjectType {
    ObjString,
    ObjFunction,
}

#[repr(C)]
#[derive(Hash, Clone)]
pub struct Object {
    pub obj_type: ObjectType,
}


impl PartialEq for Object {
    fn eq(&self, other: &Object) -> bool {
        self.obj_type == other.obj_type
    }
}

impl Eq for Object {
}

//#[cfg(feature = "debug_trace_object")]
//  mod debug_feature {
//     use crate::objects::{object::ObjectType, object_string::ObjectString, object_function::ObjectFunction};

//     use super::Object;

//     impl Drop for Object {
//         fn drop(&mut self) {
//             print!("drop object: ");
//             match self.obj_type {
//                 ObjectType::ObjString => {
//                     let object_string = std::ptr::from_mut(self) as *const ObjectString;
//                     println!("type=ObjectString, content={}", unsafe {
//                         (*object_string).content.as_str()
//                     });
//                 },
//                 ObjectType::ObjFunction => {
//                     // let object_function = std::ptr::from_mut(self) as *const ObjectFunction;
//                     // println!("type=ObjectFunction, name={}", unsafe {
//                     //     //(*object_function).chunk.code.len()
//                     //     (*object_function).name.as_str()
//                     // });
//                 }
//             }
//         }
//     }
// }
