use crate::value::{Value, ValueArray};
use std::mem::size_of;

// Forward declare concrete object structs so we can cast in dispatcher helpers.
use super::{
    object_closure::ObjectClosure,
    object_function::ObjectFunction,
    object_native_function::ObjectNativeFunction,
    object_string::ObjectString,
    object_upvalue::ObjectUpvalue,
    object_trait::ObjectTrait,
    object_struct::{ObjectStructType, ObjectStructInstance},
};

#[repr(C)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ObjectType {
    ObjString,
    ObjFunction,
    ObjNativeFunction,
    ObjClosure,
    ObjUpvalue,
    ObjTrait,
    ObjStructType,
    ObjStructInstance,
}

#[repr(C)]
#[derive(Hash, Clone, Copy)]
pub struct Object {
    pub obj_type: ObjectType,
}

pub trait NativeObject {
    fn run(&self, args: &Option<ValueArray>) -> Result<Value, String>;
}

impl Object {
    /// Shallow size (header only) – mainly for debugging.
    #[allow(dead_code)]
    pub fn shallow_size(&self) -> usize { size_of::<Object>() }

    /// Compute the deep size of the concrete object that this header belongs to.
    /// Safety: caller guarantees `self` is embedded at the start of the concrete object.
    pub unsafe fn deep_size(&self) -> usize {
        match self.obj_type {
            ObjectType::ObjString => (*(self as *const _ as *const ObjectString)).deep_size(),
            ObjectType::ObjFunction => (*(self as *const _ as *const ObjectFunction)).deep_size(),
            ObjectType::ObjNativeFunction => (*(self as *const _ as *const ObjectNativeFunction)).deep_size(),
            ObjectType::ObjClosure => (*(self as *const _ as *const ObjectClosure)).deep_size(),
            ObjectType::ObjUpvalue => (*(self as *const _ as *const ObjectUpvalue)).deep_size(),
            ObjectType::ObjTrait => (*(self as *const _ as *const ObjectTrait)).deep_size(),
            ObjectType::ObjStructType => (*(self as *const _ as *const ObjectStructType)).deep_size(),
            ObjectType::ObjStructInstance => (*(self as *const _ as *const ObjectStructInstance)).deep_size(),
        }
    }

    /// Cast helpers with debug assertions to reduce accidental UB during development.
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_string(&self) -> &ObjectString { debug_assert!(matches!(self.obj_type, ObjectType::ObjString)); &*(self as *const _ as *const ObjectString) }
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_function(&self) -> &ObjectFunction { debug_assert!(matches!(self.obj_type, ObjectType::ObjFunction)); &*(self as *const _ as *const ObjectFunction) }
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_native_function(&self) -> &ObjectNativeFunction { debug_assert!(matches!(self.obj_type, ObjectType::ObjNativeFunction)); &*(self as *const _ as *const ObjectNativeFunction) }
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_closure(&self) -> &ObjectClosure { debug_assert!(matches!(self.obj_type, ObjectType::ObjClosure)); &*(self as *const _ as *const ObjectClosure) }
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_upvalue(&self) -> &ObjectUpvalue { debug_assert!(matches!(self.obj_type, ObjectType::ObjUpvalue)); &*(self as *const _ as *const ObjectUpvalue) }
    #[inline]
    #[cfg_attr(not(feature = "gc_debug"), allow(dead_code))]
    pub unsafe fn as_trait(&self) -> &ObjectTrait { debug_assert!(matches!(self.obj_type, ObjectType::ObjTrait)); &*(self as *const _ as *const ObjectTrait) }
    #[inline]
    #[allow(dead_code)]
    pub unsafe fn as_struct_type(&self) -> &ObjectStructType { debug_assert!(matches!(self.obj_type, ObjectType::ObjStructType)); &*(self as *const _ as *const ObjectStructType) }
    #[inline]
    pub unsafe fn as_struct_instance(&self) -> &ObjectStructInstance { debug_assert!(matches!(self.obj_type, ObjectType::ObjStructInstance)); &*(self as *const _ as *const ObjectStructInstance) }
}

impl PartialEq for Object {
    fn eq(&self, other: &Object) -> bool {
        self.obj_type == other.obj_type
    }
}

impl Eq for Object {
}

/// Trait for computing heap usage of GC managed structures (owned data only).
pub trait GcSize {
    /// Bytes for the struct itself (includes inline fields, pointers, lengths, capacities meta).
    fn shallow_size(&self) -> usize;
    /// Bytes including owned heap allocations (recursive but NOT traversing to other GC objects).
    fn deep_size(&self) -> usize;
}

// Implementations for each object type. These treat referenced GC objects (by raw pointer)
// as non-owned (so only pointer size counted via the struct layout, already in shallow).

impl GcSize for ObjectString {
    fn shallow_size(&self) -> usize { size_of::<ObjectString>() }
    fn deep_size(&self) -> usize {
        // String capacity bytes (Vec<u8> internal) – use capacity not len.
        self.shallow_size() + self.content.capacity()
    }
}

impl GcSize for ObjectFunction {
    fn shallow_size(&self) -> usize { size_of::<ObjectFunction>() }
    fn deep_size(&self) -> usize {
        // name capacity + chunk deep size (Box<Chunk> heap)
        let name_bytes = self.name.capacity();
        let chunk_bytes = self.chunk.deep_size();
        self.shallow_size() + name_bytes + chunk_bytes
    }
}

impl GcSize for ObjectClosure {
    fn shallow_size(&self) -> usize { size_of::<ObjectClosure>() }
    fn deep_size(&self) -> usize {
        // Owns the upvalues Vec (capacity * usize)
        self.shallow_size() + self.upvalues.capacity() * size_of::<usize>()
    }
}

impl GcSize for ObjectNativeFunction {
    fn shallow_size(&self) -> usize { size_of::<ObjectNativeFunction>() }
    fn deep_size(&self) -> usize {
        // We cannot inspect dynamic native object internals. Approximate with box target size only.
        // Box<dyn Trait> layout: pointer + vtable pointer already inside struct (shallow). Add name capacity.
        self.shallow_size() + self.name.capacity()
    }
}

impl GcSize for ObjectUpvalue {
    fn shallow_size(&self) -> usize { size_of::<ObjectUpvalue>() }
    fn deep_size(&self) -> usize { self.shallow_size() }
}

impl GcSize for ObjectTrait {
    fn shallow_size(&self) -> usize { size_of::<ObjectTrait>() }
    fn deep_size(&self) -> usize {
        self.shallow_size() + self.name.capacity() + self.method_names.iter().map(|s| s.capacity()).sum::<usize>()
    }
}

impl GcSize for ObjectStructType {
    fn shallow_size(&self) -> usize { size_of::<ObjectStructType>() }
    fn deep_size(&self) -> usize {
    // Approximate table memory: number of entries * (string capacity + Value size)
    let table_bytes = self.field_index.iter().map(|(k, _)| k.capacity() + size_of::<crate::value::Value>()).sum::<usize>();
    self.shallow_size() + self.name.capacity() + self.field_names.iter().map(|s| s.capacity()).sum::<usize>() + table_bytes
    }
}

impl GcSize for ObjectStructInstance {
    fn shallow_size(&self) -> usize { size_of::<ObjectStructInstance>() }
    fn deep_size(&self) -> usize {
        // fields Vec capacity * Value size
        self.shallow_size() + self.fields.capacity() * size_of::<crate::value::Value>()
    }
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
