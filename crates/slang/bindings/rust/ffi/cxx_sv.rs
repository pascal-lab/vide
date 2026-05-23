use core::str;
use std::{
    ffi::{c_char, c_uchar, c_void},
    fmt::{self, Display},
    marker::PhantomData,
    mem::MaybeUninit,
};

use cxx::{ExternType, type_id};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CxxSV<'a> {
    repr: MaybeUninit<[*const c_void; 2]>,
    borrow: PhantomData<&'a [c_char]>,
}

unsafe impl<'a> ExternType for CxxSV<'a> {
    type Id = type_id!("::std::string_view");
    type Kind = cxx::kind::Trivial;
}

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/string_view.h");

        #[namespace = "std"]
        #[cxx_name = "string_view"]
        type CxxSV<'a> = super::CxxSV<'a>;

        fn string_view_from_str(s: &str) -> CxxSV<'_>;
        fn string_view_as_bytes(s: CxxSV<'_>) -> &[c_char];
    }
}

impl<'a> CxxSV<'a> {
    pub fn new(s: &'a str) -> Self {
        ffi::string_view_from_str(s)
    }

    pub fn as_bytes(self) -> &'a [c_uchar] {
        unsafe {
            std::mem::transmute::<&'a [c_char], &'a [c_uchar]>(ffi::string_view_as_bytes(self))
        }
    }
}

impl Display for CxxSV<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = unsafe { str::from_utf8_unchecked(self.as_bytes()) };
        write!(f, "{}", s)
    }
}

impl fmt::Debug for CxxSV<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}
