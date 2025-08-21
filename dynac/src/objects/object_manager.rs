use crate::objects::{
    object::{Object, NativeObject},
    object_string::ObjectString,
    object_function::ObjectFunction,
    object_closure::ObjectClosure,
    object_native_function::ObjectNativeFunction,
    object_upvalue::ObjectUpvalue,
};

#[allow(dead_code)]
pub struct ObjectManager {
    objects: Vec<*mut Object>,
}

#[allow(dead_code)]
impl ObjectManager {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    pub fn push_object(&mut self, obj: *mut Object) {
        self.objects.push(obj);
    }

    pub fn alloc_string(&mut self, value: &str) -> *mut ObjectString {
        let obj = Box::new(ObjectString::new(value));
        let ptr = Box::into_raw(obj);
        self.push_object(ptr as *mut Object);
        ptr
    }

    pub fn alloc_function(&mut self, arity: usize, name: String) -> *mut ObjectFunction {
        let obj = Box::new(ObjectFunction::new(arity as u8, name));
        let ptr = Box::into_raw(obj);
        self.push_object(ptr as *mut Object);
        ptr
    }

    pub fn alloc_closure(&mut self, function: *mut ObjectFunction) -> *mut ObjectClosure {
        let obj = Box::new(ObjectClosure::new(function));
        let ptr = Box::into_raw(obj);
        self.push_object(ptr as *mut Object);
        ptr
    }

    pub fn alloc_native_function<T: NativeObject + 'static>(&mut self, name: String, arity: usize, native_obj: T) -> *mut ObjectNativeFunction {
        let obj = Box::new(ObjectNativeFunction::new(name, arity as u8, native_obj));
        let ptr = Box::into_raw(obj);
        self.push_object(ptr as *mut Object);
        ptr
    }

    pub fn alloc_upvalue(&mut self, location: *mut crate::value::Value) -> *mut ObjectUpvalue {
        let obj = Box::new(ObjectUpvalue::new(location));
        let ptr = Box::into_raw(obj);
        self.push_object(ptr as *mut Object);
        ptr
    }

    /// Iterate over all managed objects (for GC mark/sweep)
    pub fn iter(&self) -> impl Iterator<Item = &*mut Object> {
        self.objects.iter()
    }

    /// Remove a pointer from the manager (optional, for GC sweep)
    pub fn remove_object(&mut self, ptr: *mut Object) {
        if let Some(pos) = self.objects.iter().position(|&p| p == ptr) {
            self.objects.swap_remove(pos);
        }
    }

    /// Deallocate all objects (for VM shutdown or full sweep)
    pub unsafe fn free_all(&mut self) {
        for &ptr in &self.objects {
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
        }
        self.objects.clear();
    }
}

impl Drop for ObjectManager {
    fn drop(&mut self) {
        unsafe { self.free_all(); }
    }
}
