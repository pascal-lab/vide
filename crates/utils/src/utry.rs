#[macro_export]
macro_rules! try_ {
    ($expr:expr) => {
        || -> _ { Some($expr) }()
    };
}

#[macro_export]
macro_rules! try_or_default {
    ($expr:expr) => {
        try_!($expr).unwrap_or_default()
    };
}
