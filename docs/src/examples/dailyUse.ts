export const languageFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/traffic_light.sv',
    source: `typedef enum logic [1:0] {
  RED,
  YELLOW,
  GREEN
} light_state_t;

module traffic_light (
  input  logic clk,
  input  logic rst_n,
  output light_state_t state_o
);
  light_state_t state_q;

  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      state_q <= RED;
    end else begin
      unique case (state_q)
        RED:    state_q <= GREEN;
        GREEN:  state_q <= YELLOW;
        YELLOW: state_q <= RED;
      endcase
    end
  end

  assign state_o = state_q;
endmodule
`,
  },
];

export const diagnosticFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/diagnostic_demo.sv',
    source: `module diagnostic_demo (
  input  logic [7:0] data_i,
  output logic [7:0] data_o
);
  logic [7:0] data_q;

  always_comb begin
    data_q = data_i & ;
  end

  assign data_o = data_q;
endmodule
`,
  },
];

export const navigationFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/pipeline_stage.sv',
    source: `module pipeline_stage #(
  parameter int WIDTH = 8
) (
  input  logic             clk,
  input  logic             rst_n,
  input  logic [WIDTH-1:0] data_i,
  output logic [WIDTH-1:0] data_o
);
  logic [WIDTH-1:0] data_q;

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
  {
    path: 'rtl/pipeline_top.sv',
    source: `module pipeline_top (
  input  logic        clk,
  input  logic        rst_n,
  input  logic [15:0] packet_i,
  output logic [15:0] packet_o
);
  logic [15:0] staged_packet;

  pipeline_stage #(
    .WIDTH(16)
  ) u_stage (
    .clk(clk),
    .rst_n(rst_n),
    .data_i(packet_i),
    .data_o(staged_packet)
  );

  assign packet_o = staged_packet;
endmodule
`,
  },
];

export const editAidFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/packet_fifo.sv',
    source: `module packet_fifo #(
  parameter int DEPTH = 4,
  parameter int WIDTH = 8
) (
  input  logic             clk,
  input  logic             rst_n,
  input  logic [WIDTH-1:0] data_i,
  input  logic             valid_i,
  output logic [WIDTH-1:0] data_o,
  output logic             ready_o
);
  assign data_o = data_i;
  assign ready_o = valid_i & rst_n;
endmodule
`,
  },
  {
    path: 'rtl/fifo_client.sv',
    source: `module fifo_client (
  input  logic        clk,
  input  logic        rst_n,
  input  logic [15:0] payload_i,
  input  logic        payload_valid,
  output logic [15:0] payload_o,
  output logic        payload_ready
);
  packet_fifo #(2, 16) u_fifo (
    clk,
    rst_n,
    payload_i,
    payload_valid,
    payload_o,
    payload_ready
  );
endmodule
`,
  },
];

export const formattingFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/formatting_demo.sv',
    source: `module formatting_demo(input logic clk,input logic rst_n,output logic done_o);
always_ff@(posedge clk or negedge rst_n)begin
if(!rst_n)begin
done_o<=1'b0;
end else begin
done_o<=~done_o;
end
end
endmodule
`,
  },
];

export const structureFiles = [
  {
    path: 'vizsla.toml',
    languageId: 'toml',
    source: `sources = ["rtl/**"]
`,
  },
  {
    path: 'rtl/feature_stage.sv',
    source: `module feature_stage #(
  parameter int WIDTH = 8
) (
  input  logic [WIDTH-1:0] data_i,
  output logic [WIDTH-1:0] data_o
);
  assign data_o = data_i;
endmodule
`,
  },
  {
    path: 'rtl/feature_top.sv',
    source: `module feature_top (
  input  logic [7:0] sample_i,
  output logic [7:0] sample_o
);
  logic [7:0] stage_a;
  logic [7:0] stage_b;

  feature_stage u_decode (
    .data_i(sample_i),
    .data_o(stage_a)
  );

  feature_stage u_execute (
    .data_i(stage_a),
    .data_o(stage_b)
  );

  assign sample_o = stage_b;
endmodule
`,
  },
];
