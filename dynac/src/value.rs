#[derive(Debug, PartialEq)]
pub enum ValueType {
    ValueBool,
    ValueNil,
    ValueNumber,
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

pub type ValueArray = Vec<Value>;

pub fn print_value(value: Value) {
    match value.value_type {
        ValueType::ValueNumber => {
            let real_value = as_number(&value);
            if real_value.fract() == 0.0 {
                // 如果没有小数部分，则按整数打印
                print!("{}", real_value as i64);
            } else {
                // 否则，找到最接近的有效数字进行打印
                let formatted = format!("{:.10}", real_value).trim_end_matches('0').to_string();
                let formatted = formatted.trim_end_matches('.').to_string(); // 去掉末尾多余的点
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
        _ => unreachable!("Unexpected value type: {:?}", value.value_type),
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