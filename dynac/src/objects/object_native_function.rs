use crate::{objects::object::{NativeObject, Object, ObjectType}, value::{make_nil_value, Value, ValueArray, ValueType}};

#[repr(C)]
pub struct ObjectNativeFunction {
    pub object: Object,
    pub name: String,
    pub arity: u8,
    pub native_object: Box<dyn NativeObject>,
}

impl ObjectNativeFunction {
    pub fn new(name: String, arity: u8, native_object: impl NativeObject + 'static) -> Self {
        ObjectNativeFunction {
            object: Object {
                obj_type: ObjectType::ObjNativeFunction
            },
            name,
            arity,
            native_object: Box::new(native_object),
        }
    }

    pub fn invoke(&self, args: &Option<ValueArray>) -> Result<Value, String> {
        if self.arity > 0 {
            match args {
                Some(_) => {
                },
                None => return Err(std::format!("Expect {} arguments but got 0.", self.arity).to_string()),
            }
        }
        self.native_object.run(args)
    }
}

mod debug_feature {
    use crate::objects::object_native_function::ObjectNativeFunction;

    impl Drop for ObjectNativeFunction {
        fn drop(&mut self) {
            print!("drop native function object: ");
            let object_function = std::ptr::from_mut(self) as *const ObjectNativeFunction;
            println!("type=ObjectNativeFunction, name={}", unsafe {
                (*object_function).name.as_str()
            });
        }
    }
}
