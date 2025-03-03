
pub type Value = f64;
pub type ValueArray = Vec<Value>;

pub fn print_value(value: Value) {
    print!("{:.6}", value);
}
// pub struct MyStruct {
//     data: Value,
// }

// impl MyStruct {
//     pub fn new(data: Value) -> Self {
//         MyStruct { data }
//     }
// }