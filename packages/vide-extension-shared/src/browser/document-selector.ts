export function videDocumentSelector(rootUri: string): Array<{
  scheme: "file";
  language: "verilog" | "systemverilog";
  pattern?: string;
}> {
  const rootPath = new URL(rootUri).pathname.replace(/\/+$/, "");
  const pattern = rootPath ? `${decodeURIComponent(rootPath)}/**` : undefined;

  return [
    { scheme: "file", language: "systemverilog", pattern },
    { scheme: "file", language: "verilog", pattern },
  ];
}
