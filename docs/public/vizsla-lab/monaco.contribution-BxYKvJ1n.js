import { _ as e, h as t, n } from "./editor.api2-Dx2CGLiK.js";
//#region node_modules/.pnpm/monaco-editor@0.55.1/node_modules/monaco-editor/esm/vs/language/typescript/lib/typescriptServicesMetadata.js
var r = "5.9.3", i = /* @__PURE__ */ e({
	JsxEmit: () => o,
	ModuleKind: () => a,
	ModuleResolutionKind: () => l,
	NewLineKind: () => s,
	ScriptTarget: () => c,
	getJavaScriptWorker: () => g,
	getTypeScriptWorker: () => h,
	javascriptDefaults: () => m,
	typescriptDefaults: () => p,
	typescriptVersion: () => d
}), a = /* @__PURE__ */ ((e) => (e[e.None = 0] = "None", e[e.CommonJS = 1] = "CommonJS", e[e.AMD = 2] = "AMD", e[e.UMD = 3] = "UMD", e[e.System = 4] = "System", e[e.ES2015 = 5] = "ES2015", e[e.ESNext = 99] = "ESNext", e))(a || {}), o = /* @__PURE__ */ ((e) => (e[e.None = 0] = "None", e[e.Preserve = 1] = "Preserve", e[e.React = 2] = "React", e[e.ReactNative = 3] = "ReactNative", e[e.ReactJSX = 4] = "ReactJSX", e[e.ReactJSXDev = 5] = "ReactJSXDev", e))(o || {}), s = /* @__PURE__ */ ((e) => (e[e.CarriageReturnLineFeed = 0] = "CarriageReturnLineFeed", e[e.LineFeed = 1] = "LineFeed", e))(s || {}), c = /* @__PURE__ */ ((e) => (e[e.ES3 = 0] = "ES3", e[e.ES5 = 1] = "ES5", e[e.ES2015 = 2] = "ES2015", e[e.ES2016 = 3] = "ES2016", e[e.ES2017 = 4] = "ES2017", e[e.ES2018 = 5] = "ES2018", e[e.ES2019 = 6] = "ES2019", e[e.ES2020 = 7] = "ES2020", e[e.ESNext = 99] = "ESNext", e[e.JSON = 100] = "JSON", e[e.Latest = 99] = "Latest", e))(c || {}), l = /* @__PURE__ */ ((e) => (e[e.Classic = 1] = "Classic", e[e.NodeJs = 2] = "NodeJs", e))(l || {}), u = class {
	constructor(e, t, r, i, a) {
		this._onDidChange = new n(), this._onDidExtraLibsChange = new n(), this._extraLibs = /* @__PURE__ */ Object.create(null), this._removedExtraLibs = /* @__PURE__ */ Object.create(null), this._eagerModelSync = !1, this.setCompilerOptions(e), this.setDiagnosticsOptions(t), this.setWorkerOptions(r), this.setInlayHintsOptions(i), this.setModeConfiguration(a), this._onDidExtraLibsChangeTimeout = -1;
	}
	get onDidChange() {
		return this._onDidChange.event;
	}
	get onDidExtraLibsChange() {
		return this._onDidExtraLibsChange.event;
	}
	get modeConfiguration() {
		return this._modeConfiguration;
	}
	get workerOptions() {
		return this._workerOptions;
	}
	get inlayHintsOptions() {
		return this._inlayHintsOptions;
	}
	getExtraLibs() {
		return this._extraLibs;
	}
	addExtraLib(e, t) {
		let n;
		if (n = t === void 0 ? `ts:extralib-${Math.random().toString(36).substring(2, 15)}` : t, this._extraLibs[n] && this._extraLibs[n].content === e) return { dispose: () => {} };
		let r = 1;
		return this._removedExtraLibs[n] && (r = this._removedExtraLibs[n] + 1), this._extraLibs[n] && (r = this._extraLibs[n].version + 1), this._extraLibs[n] = {
			content: e,
			version: r
		}, this._fireOnDidExtraLibsChangeSoon(), { dispose: () => {
			let e = this._extraLibs[n];
			e && e.version === r && (delete this._extraLibs[n], this._removedExtraLibs[n] = r, this._fireOnDidExtraLibsChangeSoon());
		} };
	}
	setExtraLibs(e) {
		for (let e in this._extraLibs) this._removedExtraLibs[e] = this._extraLibs[e].version;
		if (this._extraLibs = /* @__PURE__ */ Object.create(null), e && e.length > 0) for (let t of e) {
			let e = t.filePath || `ts:extralib-${Math.random().toString(36).substring(2, 15)}`, n = t.content, r = 1;
			this._removedExtraLibs[e] && (r = this._removedExtraLibs[e] + 1), this._extraLibs[e] = {
				content: n,
				version: r
			};
		}
		this._fireOnDidExtraLibsChangeSoon();
	}
	_fireOnDidExtraLibsChangeSoon() {
		this._onDidExtraLibsChangeTimeout === -1 && (this._onDidExtraLibsChangeTimeout = window.setTimeout(() => {
			this._onDidExtraLibsChangeTimeout = -1, this._onDidExtraLibsChange.fire(void 0);
		}, 0));
	}
	getCompilerOptions() {
		return this._compilerOptions;
	}
	setCompilerOptions(e) {
		this._compilerOptions = e || /* @__PURE__ */ Object.create(null), this._onDidChange.fire(void 0);
	}
	getDiagnosticsOptions() {
		return this._diagnosticsOptions;
	}
	setDiagnosticsOptions(e) {
		this._diagnosticsOptions = e || /* @__PURE__ */ Object.create(null), this._onDidChange.fire(void 0);
	}
	setWorkerOptions(e) {
		this._workerOptions = e || /* @__PURE__ */ Object.create(null), this._onDidChange.fire(void 0);
	}
	setInlayHintsOptions(e) {
		this._inlayHintsOptions = e || /* @__PURE__ */ Object.create(null), this._onDidChange.fire(void 0);
	}
	setMaximumWorkerIdleTime(e) {}
	setEagerModelSync(e) {
		this._eagerModelSync = e;
	}
	getEagerModelSync() {
		return this._eagerModelSync;
	}
	setModeConfiguration(e) {
		this._modeConfiguration = e || /* @__PURE__ */ Object.create(null), this._onDidChange.fire(void 0);
	}
}, d = r, f = {
	completionItems: !0,
	hovers: !0,
	documentSymbols: !0,
	definitions: !0,
	references: !0,
	documentHighlights: !0,
	rename: !0,
	diagnostics: !0,
	documentRangeFormattingEdits: !0,
	signatureHelp: !0,
	onTypeFormattingEdits: !0,
	codeActions: !0,
	inlayHints: !0
}, p = new u({
	allowNonTsExtensions: !0,
	target: 99
}, {
	noSemanticValidation: !1,
	noSyntaxValidation: !1,
	onlyVisible: !1
}, {}, {}, f), m = new u({
	allowNonTsExtensions: !0,
	allowJs: !0,
	target: 99
}, {
	noSemanticValidation: !0,
	noSyntaxValidation: !1,
	onlyVisible: !1
}, {}, {}, f), h = () => _().then((e) => e.getTypeScriptWorker()), g = () => _().then((e) => e.getJavaScriptWorker());
function _() {
	return import("./tsMode-CMRvdJSX.js");
}
t.onLanguage("typescript", () => _().then((e) => e.setupTypeScript(p))), t.onLanguage("javascript", () => _().then((e) => e.setupJavaScript(m)));
//#endregion
export { p as n, i as t };
