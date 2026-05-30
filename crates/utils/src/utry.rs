#[macro_export]
macro_rules! check_or_throw {
    ($expr:expr) => {
        if !($expr) {
            None?
        }
    };
}
