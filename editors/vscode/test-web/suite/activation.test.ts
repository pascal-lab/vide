import * as vscode from "vscode";

const EXTENSION_ID = "pascal-lab.vide-ide";

suite("Vide web extension smoke", () => {
  test("activates in the web host", async () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    if (!extension) {
      throw new Error(`extension ${EXTENSION_ID} is not registered`);
    }

    // Browser-host extensions activate on language contribution; opening a
    // SystemVerilog document triggers it.
    const document = await vscode.workspace.openTextDocument({
      language: "systemverilog",
      content: "module smoke; endmodule\n",
    });
    await vscode.window.showTextDocument(document);

    const deadline = Date.now() + 20_000;
    while (!extension.isActive && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }

    if (!extension.isActive) {
      throw new Error(`extension ${EXTENSION_ID} did not activate within 20s`);
    }
  });
});
