((one_line_comment) @injection.content
  (#set! injection.language "comment"))

((block_comment) @injection.content
  (#set! injection.language "comment"))

((macro_text) @injection.content
  (#set! injection.language "SystemVerilog"))
