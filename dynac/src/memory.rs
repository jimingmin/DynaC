
macro_rules! grow_capacity {
    ($capacity:expr) => {{
        if $capacity < 8 {
            8
        } else {
            $capacity * 2
        }
    }};
}
