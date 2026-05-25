//#region node_modules/.pnpm/monaco-editor@0.55.1/node_modules/monaco-editor/esm/vs/basic-languages/azcli/azcli.js
var e = { comments: { lineComment: "#" } }, t = {
	defaultToken: "keyword",
	ignoreCase: !0,
	tokenPostfix: ".azcli",
	str: /[^#\s]/,
	tokenizer: {
		root: [
			{ include: "@comment" },
			[/\s-+@str*\s*/, { cases: {
				"@eos": {
					token: "key.identifier",
					next: "@popall"
				},
				"@default": {
					token: "key.identifier",
					next: "@type"
				}
			} }],
			[/^-+@str*\s*/, { cases: {
				"@eos": {
					token: "key.identifier",
					next: "@popall"
				},
				"@default": {
					token: "key.identifier",
					next: "@type"
				}
			} }]
		],
		type: [
			{ include: "@comment" },
			[/-+@str*\s*/, { cases: {
				"@eos": {
					token: "key.identifier",
					next: "@popall"
				},
				"@default": "key.identifier"
			} }],
			[/@str+\s*/, { cases: {
				"@eos": {
					token: "string",
					next: "@popall"
				},
				"@default": "string"
			} }]
		],
		comment: [[/#.*$/, { cases: { "@eos": {
			token: "comment",
			next: "@popall"
		} } }]]
	}
};
//#endregion
export { e as conf, t as language };
