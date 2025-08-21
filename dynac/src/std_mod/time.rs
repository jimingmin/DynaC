use crate::{objects::object::NativeObject, value::{make_numer_value, Value, ValueArray}};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ClockTime;

impl NativeObject for ClockTime {
    fn run(&self, _args: &Option<ValueArray>) -> Result<Value, String> {
        println!("Called ClockTime");
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
        Ok(make_numer_value(duration.as_millis() as f64))
    }
}

impl ClockTime {
    pub fn new() -> Self {
        ClockTime{}
    }
}