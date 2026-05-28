const rootProjectConfig = `#:schema https://vide.pascal-lab.net/schemas/v1/vide.schema.json
sources = ["*.v"]

top_modules = ["top"]
`;

const defaultTemplateProjectConfig = `#:schema https://vide.pascal-lab.net/schemas/v1/vide.schema.json
sources = ["*.v"]

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
`;

const manifestFile = (source = rootProjectConfig) => ({
  path: 'vide.toml',
  languageId: 'toml',
  source,
});

export const syntaxHighlightingFiles = [
  manifestFile(),
  {
    path: 'usage.v',
    source: `module pulse_sync (
    input  wire clk,
    input  wire rst_n,
    input  wire pulse_in,
    output reg  pulse_out
);
    reg pulse_d;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pulse_d   <= 1'b0;
            pulse_out <= 1'b0;
        end else begin
            pulse_d   <= pulse_in;
            pulse_out <= pulse_in & ~pulse_d;
        end
    end
endmodule

module top (
    input  wire clk,
    input  wire rst_n,
    input  wire button,
    output wire led
);
    wire one_cycle_pulse;

    pulse_sync u_pulse_sync (
        .clk(clk),
        .rst_n(rst_n),
        .pulse_in(button),
        .pulse_out(one_cycle_pulse)
    );

    assign led = one_cycle_pulse;
endmodule
`,
  },
];

export const usageFiles = syntaxHighlightingFiles;

export const completionFiles = [
  manifestFile(),
  {
    path: 'completion.v',
    source: `module adder8 (
    input  wire [7:0] lhs,
    input  wire [7:0] rhs,
    output wire [7:0] sum
);
    assign sum = lhs + rhs;
endmodule

module top (
    input  wire [7:0] a,
    input  wire [7:0] b,
    output wire [7:0] y
);
    wire [7:0] result;

    adder8 u0(
        .lhs(a),
        .rhs(b),
        .sum(result)
    );

    assign y = result;

endmodule
`,
  },
];

export const slangDiagnosticFiles = [
  manifestFile(),
  {
    path: 'diagnostics.v',
    source: `\`default_nettype none

module top (
    input  wire clk,
    input  wire rst_n,
    input  wire valid,
    output wire ready
);
    reg [3:0] counter;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            counter <= 4'd0;
        end else if (valid) begin
            counter <= counter + missing_step;
        end
    end

    assign ready = counter[0] & missing_enable;
endmodule

\`default_nettype wire
`,
  },
];

export const missingPortFiles = [
  manifestFile(defaultTemplateProjectConfig),
  {
    path: 'missing_port_example.v',
    source: `module child (
    input  wire a,
    input  wire b,
    output wire y
);
    assign y = a & b;
endmodule

module top (
    input  wire a,
    input  wire b,
    output wire y
);
    child u_child (
        .a(a),
        .b(b)
    );
endmodule
`,
  },
];

export const diagnosticFiles = missingPortFiles;

export const navigationFiles = [
  manifestFile(),
  {
    path: 'and_gate.v',
    source: `module and_gate (
    input  wire a,
    input  wire b,
    output wire y
);
    assign y = a & b;
endmodule
`,
  },
  {
    path: 'top.v',
    source: `module top (
    input  wire sw0,
    input  wire sw1,
    output wire led0
);
    wire gate_out;

    and_gate u_and_gate (
        .a(sw0),
        .b(sw1),
        .y(gate_out)
    );

    assign led0 = gate_out;
endmodule
`,
  },
];

export const hoverFiles = [
  manifestFile(),
  {
    path: 'hover.v',
    source: `module blinker (
    input  wire clk,
    input  wire rst_n,
    output wire [7:0] led
);
    assign led = rst_n ? 8'hA5 : 8'h00;
endmodule

module top (
    input  wire clk,
    input  wire rst_n,
    output wire [7:0] led
);
    localparam [3:0] MODE  = 4'b1010;
    localparam [7:0] MASK  = 8'hF0;
    localparam [15:0] LIMIT = 16'd255;

    blinker u_blinker (
        .clk(clk),
        .rst_n(rst_n),
        .led(led)
    );
endmodule
`,
  },
];

export const semanticTokenFiles = [
  manifestFile(defaultTemplateProjectConfig),
  {
    path: 'semantic-token-demo.v',
    source: `module semantic_tokens_demo (
    input  logic clk,
    input  logic rst_n,
    input  logic [7:0] data_i,
    output logic [7:0] data_o
);
    logic [7:0] data_q;

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            data_q <= '0;
        end else begin
            data_q <= data_i;
        end
    end

    assign data_o = data_q;
endmodule


`,
  },
];

export const codeActionRenameFiles = [
  manifestFile(),
  {
    path: 'code_action_rename.v',
    source: `module counter (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       tick,
    output reg  [3:0] count
);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            count <= 4'd0;
        end else if (tick) begin
            count <= count + 4'd1;
        end
    end
endmodule

module top (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       button,
    output wire [3:0] leds
);
    wire [3:0] sample_count;

    counter u_counter (
        .clk(clk),
        .rst_n(rst_n),
        .count(sample_count),
        .tick(button)
    );

    assign leds = sample_count;
endmodule
`,
  },
];

export const editAidFiles = [
  manifestFile(),
  {
    path: 'code_action_rename.v',
    source: `module counter (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       tick,
    output reg  [3:0] count
);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            count <= 4'd0;
        end else if (tick) begin
            count <= count + 4'd1;
        end
    end
endmodule

module top (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       button,
    output wire [3:0] leds
);
    wire [3:0] smp_cnt;

    counter u_counter (
        .clk(clk),
        .rst_n(rst_n),
        .count(smp_cnt),
        .tick(button)
    );

    assign leds = smp_cnt;
endmodule
`,
  },
];

export const inlayHintFiles = [
  manifestFile(),
  {
    path: 'inlay_hints.v',
    source: `module mux2 #(
    parameter WIDTH = 8
) (
    input  wire [WIDTH-1:0] lhs,
    input  wire [WIDTH-1:0] rhs,
    input  wire             sel,
    output wire [WIDTH-1:0] out
);
    assign out = sel ? rhs : lhs;
endmodule

module top (
    input  wire [7:0] a,
    input  wire [7:0] b,
    input  wire       choose_b,
    output wire [7:0] y
);
    mux2 #(8) u_mux (a, b, choose_b, y);
endmodule
`,
  },
];

export const structureFiles = [
  manifestFile(),
  {
    path: 'inlay_hints.v',
    source: `module mux2 #(
    parameter WIDTH = 8
) (
    input  wire [WIDTH-1:0] lhs,
    input  wire [WIDTH-1:0] rhs,
    input  wire             sel,
    output wire [WIDTH-1:0] out
);
    assign out = sel ? rhs : lhs;
endmodule

module top (
    input  wire [7:0] a,
    input  wire [7:0] b,
    input  wire       choose_b,
    output wire [7:0] y
);
    mux2 #(.WIDTH(8)) u_mux (
        .lhs(a),
        .rhs(b),
        .sel(choose_b),
        .out(y)
    );
endmodule
`,
  },
];

export const documentSymbolFiles = [
  manifestFile(),
  {
    path: 'document_symbol.v',
    source: `module packet_counter #(
    parameter WIDTH = 8
) (
    input  wire             clk,
    input  wire             rst_n,
    input  wire             valid,
    output wire [WIDTH-1:0] count
);
    localparam [WIDTH-1:0] RESET_VALUE = 0;

    reg [WIDTH-1:0] count_q;

    function [WIDTH-1:0] next_count;
        input [WIDTH-1:0] current;
        input             step;
        begin
            next_count = step ? current + 1'b1 : current;
        end
    endfunction

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            count_q <= RESET_VALUE;
        end else begin
            count_q <= next_count(count_q, valid);
        end
    end

    assign count = count_q;
endmodule

module top (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       valid,
    output wire [7:0] count
);
    packet_counter #(8) u_counter (
        .clk(clk),
        .rst_n(rst_n),
        .valid(valid),
        .count(count)
    );
endmodule
`,
  },
];
