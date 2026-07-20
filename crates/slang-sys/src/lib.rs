pub mod ffi;

pub fn parse_root_kind(text: &str) -> u16 {
    ffi::parse_root_kind(text)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_calls_upstream_slang_parser() {
        let root_kind = crate::parse_root_kind("module demo; endmodule");

        assert_ne!(root_kind, 0);
    }
}
