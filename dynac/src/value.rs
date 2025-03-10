
pub type Value = f64;
pub type ValueArray = Vec<Value>;

pub fn print_value(value: Value) {
    if value.fract() == 0.0 {
        // 如果没有小数部分，则按整数打印
        print!("{}", value as i64);
    } else {
        // 否则，找到最接近的有效数字进行打印
        let formatted = format!("{:.10}", value).trim_end_matches('0').to_string();
        let formatted = formatted.trim_end_matches('.').to_string(); // 去掉末尾多余的点
        print!("{}", formatted);
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