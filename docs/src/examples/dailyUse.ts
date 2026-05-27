const rootProjectConfig = `#:schema https://vide.pascal-lab.net/schemas/v1/vide.schema.json
sources = ["*.v"]

top_modules = ["top"]
`;

export const completionFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: rootProjectConfig,
  },
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

export const diagnosticFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: rootProjectConfig,
  },
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

export const navigationFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: rootProjectConfig,
  },
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

export const semanticTokenFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: `#:schema https://vide.pascal-lab.net/schemas/v1/vide.schema.json
sources = ["*.v"]

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
`,
  },
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

export const editAidFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: rootProjectConfig,
  },
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

export const structureFiles = [
  {
    path: 'vide.toml',
    languageId: 'toml',
    source: rootProjectConfig,
  },
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
