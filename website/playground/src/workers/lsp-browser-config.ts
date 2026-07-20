export function browserClientCapabilities(): Record<string, unknown> {
  return {
    textDocument: {
      codeAction: {
        codeActionLiteralSupport: {
          codeActionKind: {
            valueSet: ["", "quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite", "source"],
          },
        },
        resolveSupport: { properties: ["edit", "command"] },
      },
      codeLens: {},
      completion: {
        completionItem: {
          insertReplaceSupport: true,
          labelDetailsSupport: true,
          snippetSupport: true,
        },
      },
      declaration: { linkSupport: true },
      definition: { linkSupport: true },
      diagnostic: {},
      documentHighlight: {},
      documentSymbol: { hierarchicalDocumentSymbolSupport: true },
      foldingRange: { lineFoldingOnly: true },
      hover: { contentFormat: ["markdown", "plaintext"] },
      inlayHint: {
        dynamicRegistration: false,
        resolveSupport: { properties: ["tooltip", "textEdits", "label.tooltip", "label.location", "label.command"] },
      },
      references: {},
      rename: { prepareSupport: true },
      semanticTokens: {
        dynamicRegistration: false,
        formats: ["relative"],
        multilineTokenSupport: false,
        overlappingTokenSupport: false,
        requests: {
          full: { delta: true },
          range: true,
        },
        tokenModifiers: [
          "declaration",
          "definition",
          "readonly",
          "static",
          "deprecated",
          "abstract",
          "async",
          "modification",
          "documentation",
          "defaultLibrary",
          "read",
          "write",
          "ref",
        ],
        tokenTypes: [
          "comment",
          "decorator",
          "enumMember",
          "enum",
          "function",
          "interface",
          "keyword",
          "macro",
          "method",
          "namespace",
          "number",
          "operator",
          "parameter",
          "property",
          "string",
          "struct",
          "typeParameter",
          "variable",
          "type",
          "port_clock",
          "port_reset",
          "port_generic",
          "instance",
          "type_alias",
          "generic",
        ],
      },
      signatureHelp: {
        signatureInformation: {
          documentationFormat: ["markdown", "plaintext"],
          parameterInformation: { labelOffsetSupport: true },
        },
      },
      synchronization: { didSave: true, dynamicRegistration: false },
      typeDefinition: { linkSupport: true },
    },
    workspace: {
      codeLens: { refreshSupport: true },
      configuration: false,
      diagnostic: { refreshSupport: true },
      inlayHint: { refreshSupport: true },
      workspaceFolders: false,
    },
  };
}

export function browserInitializationOptions(): Record<string, unknown> {
  return {
    files: {
      excludeDirs: [],
      watcher: "client",
    },
    workspace: {
      auto: { reload: true },
    },
    scope: {
      visibility: "private",
    },
    formatter: {
      provider: "verible",
      path: null,
      args: ["--failsafe_success=false"],
    },
    formatting: {
      on: { enter: true },
      in: { comments: true },
      indent: { width: 4 },
    },
    inlayHints: {
      port: { connection: { enable: true } },
      parameter: { assignment: { enable: true } },
      end: { structure: { enable: true } },
    },
    lens: {
      instantiations: { enable: true },
    },
    semantic: {
      tokens: {
        port: {
          clk: { rst: { enable: true } },
          input: { output: { enable: true } },
        },
      },
    },
    diagnostics: {
      enable: true,
      update: "onSave",
      parse: { enable: true },
      semantic: { enable: true },
      slang: {
        warnings: [],
        rules: [],
      },
    },
    signature: {
      help: { params: { only: false } },
    },
    qihe: {
      command: "qihe",
      autoConfigureArgsFromManifest: true,
      compileArgs: [],
      runArgs: ["-g", "std"],
    },
  };
}
