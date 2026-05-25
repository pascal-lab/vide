#[macro_export]
macro_rules! try_ {
    ($expr:expr) => {
        || -> _ { Some($expr) }()
    };

    ($($tt:tt)*) => {
        try_!{{$($tt)*}}
    };
}

#[macro_export]
macro_rules! try_or_default {
    ($expr:expr) => {
        try_!($expr).unwrap_or_default()
    };


    ($($tt:tt)*) => {
        try_or_default!{{$($tt)*}}
    };
}

#[macro_export]
macro_rules! check_or_throw {
    ($expr:expr) => {
        if !($expr) {
            None?
        }
    };
}
