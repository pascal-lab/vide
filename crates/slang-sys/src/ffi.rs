#![allow(non_snake_case)]
#![allow(clippy::module_inception)]
#![allow(clippy::too_many_arguments)]

#[cxx::bridge(namespace = "slang_sys")]
mod slang_ffi {
    unsafe extern "C++" {
        include!("wrapper.h");

        fn parse_root_kind(text: &str) -> u16;
    }
}

pub fn parse_root_kind(text: &str) -> u16 {
    slang_ffi::parse_root_kind(text)
}
