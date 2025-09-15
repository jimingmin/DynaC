use crate::objects::{
    object::{Object, NativeObject},
    object_string::ObjectString,
    object_function::ObjectFunction,
    object_closure::ObjectClosure,
    object_native_function::ObjectNativeFunction,
    object_upvalue::ObjectUpvalue,
    object_trait::ObjectTrait,
    object_struct::{ObjectStructType, ObjectStructInstance},
};

#[allow(dead_code)]
pub struct ObjectManager {
    objects: Vec<*mut Object>,
    // Bytes allocated since last drain (deep size of each object when added)
    pending_bytes: usize,
}

#[allow(dead_code)]
impl ObjectManager {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            pending_bytes: 0,
        }
    }

    /// Drain and return bytes allocated since last call.
    pub fn drain_pending_bytes(&mut self) -> usize {
        let b = self.pending_bytes;
        self.pending_bytes = 0;
        b
    }

    /// Push a newly allocated object pointer, record its deep size, and return that size.
    pub fn push_object(&mut self, obj: *mut Object) -> usize {
        let size = unsafe { (*obj).deep_size() } as usize;
        self.pending_bytes += size;
        self.objects.push(obj);
        size
    }

    pub fn alloc_string(&mut self, value: &str) -> (*mut ObjectString, usize) {
        let obj = Box::new(ObjectString::new(value));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_function(&mut self, arity: usize, name: String) -> (*mut ObjectFunction, usize) {
        let obj = Box::new(ObjectFunction::new(arity as u8, name));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_closure(&mut self, function: *mut ObjectFunction) -> (*mut ObjectClosure, usize) {
        let obj = Box::new(ObjectClosure::new(function));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_native_function<T: NativeObject + 'static>(&mut self, name: String, arity: usize, native_obj: T) -> (*mut ObjectNativeFunction, usize) {
        let obj = Box::new(ObjectNativeFunction::new(name, arity as u8, native_obj));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_upvalue(&mut self, location: *mut crate::value::Value) -> (*mut ObjectUpvalue, usize) {
        let obj = Box::new(ObjectUpvalue::new(location));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_trait(&mut self, name: String) -> (*mut ObjectTrait, usize) {
        let obj = Box::new(ObjectTrait::new(name));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_struct_type(&mut self, name: String) -> (*mut ObjectStructType, usize) {
        let obj = Box::new(ObjectStructType::new(name));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    pub fn alloc_struct_instance(&mut self, struct_type: *mut ObjectStructType, field_count: usize) -> (*mut ObjectStructInstance, usize) {
        let obj = Box::new(ObjectStructInstance::new(struct_type, field_count));
        let ptr = Box::into_raw(obj);
        let size = self.push_object(ptr as *mut Object);
        (ptr, size)
    }

    /// Iterate over all managed objects (for GC mark/sweep)
    pub fn iter(&self) -> impl Iterator<Item = &*mut Object> { self.objects.iter() }

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
