
pub fn size_of<T>(_: &T) -> usize {
    const SIZE: usize = std::mem::size_of::<T>();
    SIZE
}

