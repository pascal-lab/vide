pub mod ffi;

pub fn parse_root_kind(text: &str) -> u16 {
    ffi::parse_root_kind(text)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_calls_upstream_slang_parser() {
        let test_verilog_code = r#"
module demo(
    input wire a,
    output wire b
);
begin
    assign b = a;
end
endmodule
        "#;
        let root_kind = crate::parse_root_kind(test_verilog_code);

        assert_ne!(root_kind, 0);
    }
}
