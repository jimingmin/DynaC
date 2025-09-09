use crate::objects::{object::{Object, ObjectType}, object_closure::ObjectClosure, object_function::{ObjectFunction}, object_manager::ObjectManager, object_native_function::ObjectNativeFunction, object_string::ObjectString, object_upvalue::ObjectUpvalue};
use crate::table::Table;

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ValueType {
    ValueBool,
    ValueNil,
    ValueNumber,
    ValueObject,
    ValueStackStruct, // stack lifetime struct (frame-local)
}

impl Copy for ValueType {}
impl Clone for ValueType {
    fn clone(&self) -> Self {
        *self
    }
}

pub union ValueUnion {
    pub boolean: bool,
    pub number: f64,
    pub object: *mut Object,
    pub stack_index: usize, // index into frame-local stack struct store
}

impl Copy for ValueUnion {}
impl Clone for ValueUnion {
    fn clone(&self) -> Self {
        *self
    }
}

//pub type Value = f64;
pub struct Value {
    pub value_type: ValueType,
    pub value_as: ValueUnion,
}

impl Copy for Value {}
impl Clone for Value {
    fn clone(&self) -> Self {
        *self
    }
}

#[allow(dead_code)]
impl Value {
    pub fn new() -> Self {
        // Default to nil
        Self {
            value_type: ValueType::ValueNil,
            value_as: ValueUnion { number: 0.0 } // Safe to use any field when nil
        }
    }

    /// Deep-clone a Value using the provided `ObjectManager` for any heap allocations.
    /// This replaces the previous two-function approach and centralizes the managed
    /// deep-clone behavior on `deep_clone` itself.
    pub fn deep_clone(&self, object_manager: &mut ObjectManager) -> Self {
        unsafe {
            match self.value_type {
                ValueType::ValueBool => Value {
                    value_type: self.value_type,
                    value_as: ValueUnion { boolean: self.value_as.boolean },
                },
                ValueType::ValueNil => Value {
                    value_type: self.value_type,
                    value_as: ValueUnion { boolean: false },
                },
                ValueType::ValueNumber => Value {
                    value_type: self.value_type,
                    value_as: ValueUnion { number: self.value_as.number },
                },
                ValueType::ValueObject => {
                    if self.value_as.object.is_null() {
                        return Value { value_type: self.value_type, value_as: ValueUnion { object: std::ptr::null_mut() } };
                    }

                    let object = &*self.value_as.object;
                    match object.obj_type {
                        ObjectType::ObjString => {
                            let original = &*(self.value_as.object as *const ObjectString);
                            let (new_ptr, _sz) = object_manager.alloc_string(original.content.as_str());
                            Value { value_type: self.value_type, value_as: ValueUnion { object: new_ptr as *mut Object } }
                        }

                        ObjectType::ObjFunction => {
                            let original = &*(self.value_as.object as *const ObjectFunction);
                            // allocate new function via manager and copy internals
                            let (func_ptr, _sz) = object_manager.alloc_function(original.arity as usize, original.name.clone());
                            (*func_ptr).chunk = Box::new((*original.chunk).clone());
                            (*func_ptr).upvalue_count = original.upvalue_count;
                            Value { value_type: self.value_type, value_as: ValueUnion { object: func_ptr as *mut Object } }
                        }

                        ObjectType::ObjClosure => {
                            let original = &*(self.value_as.object as *const ObjectClosure);
                            // deep-clone the referenced function first
                            let orig_func = &*original.function;
                            let (new_func_ptr, _sz_fn) = object_manager.alloc_function(orig_func.arity as usize, orig_func.name.clone());
                            (*new_func_ptr).chunk = Box::new((*orig_func.chunk).clone());
                            (*new_func_ptr).upvalue_count = orig_func.upvalue_count;

                            // allocate closure referencing new function
                            let (closure_ptr, _sz_cl) = object_manager.alloc_closure(new_func_ptr);
                            // copy upvalue indices
                            for &idx in original.upvalues.iter() {
                                (*closure_ptr).upvalues.push(idx);
                            }
                            Value { value_type: self.value_type, value_as: ValueUnion { object: closure_ptr as *mut Object } }
                        }

                        ObjectType::ObjUpvalue => {
                            let original = &*(self.value_as.object as *const ObjectUpvalue);
                            let (new_up, _sz_up) = object_manager.alloc_upvalue(original.location);
                            // copy closed value
                            (*new_up).closed = original.closed.clone();
                            // if original was already closed (location points to original.closed),
                            // update new location to point to new_up.closed
                            let orig_self_ptr = self.value_as.object as *const ObjectUpvalue;
                            let orig_closed_ptr = &(*orig_self_ptr).closed as *const Value as *mut Value;
                            if original.location == orig_closed_ptr {
                                (*new_up).location = &mut (*new_up).closed as *mut Value;
                            }
                            Value { value_type: self.value_type, value_as: ValueUnion { object: new_up as *mut Object } }
                        }

                        ObjectType::ObjNativeFunction => {
                            // Can't deep-clone trait objects generically; return shallow copy.
                            Value { value_type: self.value_type, value_as: ValueUnion { object: self.value_as.object } }
                        }
                        ObjectType::ObjTrait => {
                            // Traits are immutable metadata; shallow copy pointer.
                            Value { value_type: self.value_type, value_as: ValueUnion { object: self.value_as.object } }
                        }
                        ObjectType::ObjStructType => {
                            // Metadata only, shallow copy
                            Value { value_type: self.value_type, value_as: ValueUnion { object: self.value_as.object } }
                        }
                        ObjectType::ObjStructInstance => {
                            // Shallow copy pointer (instances are mutable; deep clone semantics TBD)
                            Value { value_type: self.value_type, value_as: ValueUnion { object: self.value_as.object } }
                        }
                    }
                }
                ValueType::ValueStackStruct => {
                    // Stack structs are not deep-cloned here (alias semantics). Shallow copy index.
                    Value { value_type: ValueType::ValueStackStruct, value_as: ValueUnion { stack_index: unsafe { self.value_as.stack_index } } }
                }
            }
        }
    }

    
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.value_type != other.value_type {
            return false;
        }

        unsafe {
            match self.value_type {
                ValueType::ValueBool => self.value_as.boolean == other.value_as.boolean,
                ValueType::ValueNumber => {
                    (self.value_as.number - other.value_as.number).abs() < f64::EPSILON
                }
                ValueType::ValueObject => {
                    self.value_as.object == other.value_as.object
                }
                ValueType::ValueStackStruct => {
                    self.value_as.stack_index == other.value_as.stack_index
                }
                ValueType::ValueNil => true,
            }
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.value_type != other.value_type {
            return None;
        }

        match self.value_type {
            ValueType::ValueBool => None,
            ValueType::ValueNil => None,
            ValueType::ValueObject => {
                let a = unsafe {
                    self.value_as.object
                };

                let b = unsafe {
                    other.value_as.object
                };

                if a == b {
                    Some(std::cmp::Ordering::Equal)
                } else if a > b {
                    Some(std::cmp::Ordering::Greater)
                } else {
                    Some(std::cmp::Ordering::Less)
                }
            }
            ValueType::ValueNumber => {
                let a = unsafe {
                    self.value_as.number
                };

                let b = unsafe {
                    other.value_as.number
                };

                if (a - b).abs() < f64::EPSILON {
                    Some(std::cmp::Ordering::Equal)
                } else if a > b {
                    Some(std::cmp::Ordering::Greater)
                } else {
                    Some(std::cmp::Ordering::Less)
                }
            }
            ValueType::ValueStackStruct => None,
        }
    }
}

#[inline(always)]
pub fn is_bool(value: &Value) -> bool {
    value.value_type == ValueType::ValueBool
}

#[inline(always)]
pub fn is_nil(value: &Value) -> bool {
    value.value_type == ValueType::ValueNil
}

#[inline(always)]
pub fn is_number(value: &Value) -> bool {
    value.value_type == ValueType::ValueNumber
}

#[inline(always)]
pub fn is_object(value: &Value) -> bool {
    value.value_type == ValueType::ValueObject
}

#[inline(always)]
pub fn is_stack_struct(value: &Value) -> bool { value.value_type == ValueType::ValueStackStruct }

#[inline(always)]
pub fn is_string(value: &Value) -> bool {
    unsafe {
        is_object(value) && (*as_object(value)).obj_type == ObjectType::ObjString
    }
}

#[inline(always)]
pub fn is_function(value: &Value) -> bool {
    unsafe {
        is_object(value) && (*as_object(value)).obj_type == ObjectType::ObjFunction
    }
}

#[inline(always)]
pub fn is_native_function(value: &Value) -> bool {
    unsafe {
        is_object(value) && (*as_object(value)).obj_type == ObjectType::ObjNativeFunction
    }
}

#[inline(always)]
pub fn is_closure(value: &Value) -> bool {
    unsafe {
        is_object(value) && (*as_object(value)).obj_type == ObjectType::ObjClosure
    }
}

#[inline(always)]
pub fn as_bool(value: &Value) -> bool {
    if value.value_type == ValueType::ValueBool {
        return unsafe {
            value.value_as.boolean
        };   
    }
    panic!("Unexpected value type. {:?}", value.value_type);
}

#[inline(always)]
pub fn as_number(value: &Value) -> f64 {
    if value.value_type == ValueType::ValueNumber {
        return unsafe {
            value.value_as.number
        };   
    }
    panic!("Unexpected value type. {:?}", value.value_type);
}

#[inline(always)]
pub fn as_object(value: &Value) -> *const Object {
    if value.value_type == ValueType::ValueObject {
        return unsafe {
            value.value_as.object
        };   
    }
    panic!("Unexpected value type. {:?}", value.value_type);
}

#[inline(always)]
#[allow(dead_code)]
pub fn as_mutable_object(value: &Value) -> *mut Object {
    if value.value_type == ValueType::ValueObject {
        return unsafe {
            value.value_as.object
        };   
    }
    panic!("Unexpected value type. {:?}", value.value_type);
}

#[inline(always)]
pub fn as_string_object(value: &Value) -> *const ObjectString {
    as_object(value) as *const ObjectString
}

#[inline(always)]
pub fn as_function_object(value: &Value) -> *const ObjectFunction {
    as_object(value) as *const ObjectFunction
}

#[inline(always)]
pub fn as_native_function_object(value: &Value) -> *const ObjectNativeFunction {
    as_object(value) as *const ObjectNativeFunction
}

#[inline(always)]
pub fn as_closure_object(value: &Value) -> *const ObjectClosure {
    as_object(value) as *const ObjectClosure
}

#[inline(always)]
pub fn make_bool_value(value: bool) -> Value {
    Value {
        value_type: ValueType::ValueBool,
        value_as: ValueUnion{boolean: value},
    }
}

#[inline(always)]
pub fn make_nil_value() -> Value {
    Value {
        value_type: ValueType::ValueNil,
        value_as: ValueUnion{number: 0.0},
    }
}

#[inline(always)]
pub fn make_numer_value(value: f64) -> Value {
    Value {
        value_type: ValueType::ValueNumber,
        value_as: ValueUnion{number: value},
    }
}

pub fn make_string_value(object_manager: &mut ObjectManager, intern_strings: &mut Table, str_value: &str) -> Value {
    if let Some(value) = intern_strings.find(str_value) {
        value.clone()
    } else {
        let (object_string, _size) = object_manager.alloc_string(str_value);
        let value = Value {
            value_type: ValueType::ValueObject,
            value_as: ValueUnion{object: object_string as *mut Object},
        };
        intern_strings.insert(str_value.to_string(), value);
        value.clone()
    }
}

pub fn make_function_value(function: *mut ObjectFunction) -> Value {
    Value {
        value_type: ValueType::ValueObject,
        value_as: ValueUnion {
            object: function as *mut Object
        }
    }
}

pub fn make_native_function_value(function: *mut ObjectNativeFunction) -> Value {
    Value {
        value_type: ValueType::ValueObject,
        value_as: ValueUnion {
            object: function as *mut Object
        }
    }
}

pub fn make_closure_value(closure: *mut ObjectClosure) -> Value {
    Value {
        value_type: ValueType::ValueObject,
        value_as: ValueUnion {
            object: closure as *mut Object
        }
    }
}

#[inline(always)]
#[allow(dead_code)]
pub fn make_upvalue(upvalue: *mut ObjectUpvalue) -> Value {
    Value {
        value_type: ValueType::ValueNumber,
        value_as: ValueUnion {
            object: upvalue as *mut Object
        }
    }
}

pub type ValueArray = Vec<Value>;

pub fn print_value(value: &Value) {
    match value.value_type {
        ValueType::ValueNumber => {
            let real_value = as_number(&value);
            if real_value.fract() == 0.0 {
                print!("{}", real_value as i64);
            } else {
                let formatted = format!("{:.10}", real_value).trim_end_matches('0').to_string();
                let formatted = formatted.trim_end_matches('.').to_string();
                print!("{}", formatted);
            }
        }
        ValueType::ValueBool => {
            if as_bool(&value) {
                print!("true");
            } else {
                print!("false");
            }
        }
        ValueType::ValueNil => {
            print!("nil");
        }
        ValueType::ValueObject => {
            print_object(value);
        }
        ValueType::ValueStackStruct => {
            print!("<stack struct>");
        }
    // all ValueType variants are handled above
    }

}

fn print_object(value: &Value) {
    unsafe {
        let object_ptr = as_object(value);
        match (*object_ptr).obj_type {
            ObjectType::ObjString => {
                let object_string = &*(object_ptr as *const ObjectString);
                print!("{}", object_string.content);
            },
            ObjectType::ObjFunction => {
                let object_function = &*(object_ptr as *const ObjectFunction);
                if object_function.name.is_empty() {
                    print!("<script>");
                    return;
                }
                print!("<fn {}>", object_function.name);
            },
            ObjectType::ObjNativeFunction => {
                let object_function = &*(object_ptr as *const ObjectNativeFunction);
                print!("<native fn {}>", object_function.name);
            },
            ObjectType::ObjClosure => {
                let closure = &*(object_ptr as *const ObjectClosure);
                print!("<closure {}>", (*closure.function).name);
            },
            ObjectType::ObjUpvalue => {
                print!("<upvalue>")
            },
            ObjectType::ObjTrait => {
                let trait_obj = &*(object_ptr as *const crate::objects::object_trait::ObjectTrait);
                print!("<trait {}>", trait_obj.name);
            },
            ObjectType::ObjStructType => {
                let s_type = &*(object_ptr as *const crate::objects::object_struct::ObjectStructType);
                print!("<struct {}>", s_type.name);
            },
            ObjectType::ObjStructInstance => {
                let inst = &*(object_ptr as *const crate::objects::object_struct::ObjectStructInstance);
                let s_type = unsafe { &*inst.struct_type };
                print!("<{} instance>", s_type.name);
            }
        }
    }

}
// pub struct MyStruct {
//     data: Value,
// }

// impl MyStruct {
//     pub fn new(data: Value) -> Self {
//         MyStruct { data }
//     }
// }