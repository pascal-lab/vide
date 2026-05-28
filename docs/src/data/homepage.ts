import type { ImageMetadata } from 'astro';

import completionItemsImage from '../assets/homepage-features/completion-items.png';
import completionModuleDeclImage from '../assets/homepage-features/completion-module-decl.png';
import completionModuleSnippetExpandedImage from '../assets/homepage-features/completion-module-snippets-expanded.png';
import completionPortsImage from '../assets/homepage-features/completion-ports.png';
import completionSnippetModuleImage from '../assets/homepage-features/completion-snippets-module.png';
import diagnosticsLoopAnalysisImage from '../assets/homepage-features/diagnostics-loop-analysis.jpeg';
import diagnosticsUndeclaredIdentifiersImage from '../assets/homepage-features/diagnostics-undeclared-identifiers.jpeg';
import documentSymbolImage from '../assets/homepage-features/document-symbol.jpeg';
import findAllReferencesImage from '../assets/homepage-features/find-all-references.jpeg';
import hoverInstanceNameImage from '../assets/homepage-features/hover-on-instance-name.png';
import hoverModuleNameImage from '../assets/homepage-features/hover-on-module-name.png';
import hoverNumberLiteralImage from '../assets/homepage-features/hover-on-number-literal.png';
import inlayHintsImage from '../assets/homepage-features/inlay-hints.png';
import missingPortsImage from '../assets/homepage-features/missing-ports.png';
import peekDefinitionImage from '../assets/homepage-features/peek-definition.png';
import renameImage from '../assets/homepage-features/rename-updated.png';

export type HomepageLocale = 'zh' | 'en';
export type HomepageFeatureLayout = 'image-left' | 'image-right';

export interface HomepageFeatureImage {
  src: ImageMetadata;
  alt: string;
}

export interface HomepageFeature {
  layout: HomepageFeatureLayout;
  eyebrow: string;
  title: string;
  description: string;
  images: HomepageFeatureImage[];
}

export type ComparisonFeatureValue = boolean | string;

export interface ComparisonColumn {
  key: string;
  label: string;
  href?: string;
}

export interface ComparisonProduct {
  name: string;
  meta: string;
  highlighted?: boolean;
  betaFeatureKeys?: readonly ComparisonFeatureKey[];
  features: Record<ComparisonFeatureKey, ComparisonFeatureValue>;
}

const escapeHtml = (value: string) =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');

const accent = (text: string) =>
  `<span class="vide-feature-carousel__description-accent">${escapeHtml(text)}</span>`;

const externalLink = (href: string, text: string) =>
  `<a class="vide-feature-carousel__description-link" href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">${escapeHtml(text)}</a>`;

const docsLink = (slug: string) => `./user-guide/features/${slug}/`;

export const normalizeHomepageLocale = (locale?: string): HomepageLocale =>
  locale?.startsWith('en') ? 'en' : 'zh';

const featureImages = {
  navigation: [
    { src: peekDefinitionImage, alt: { zh: 'Peek Definition 截图', en: 'Peek Definition screenshot' } },
    {
      src: findAllReferencesImage,
      alt: { zh: 'Find All References 截图', en: 'Find All References screenshot' },
    },
    { src: documentSymbolImage, alt: { zh: 'Document Symbol 截图', en: 'Document Symbol screenshot' } },
  ],
  insight: [
    { src: hoverModuleNameImage, alt: { zh: '模块 Hover 信息截图', en: 'Module hover screenshot' } },
    {
      src: hoverInstanceNameImage,
      alt: { zh: '例化 Hover 信息截图', en: 'Instance hover screenshot' },
    },
    {
      src: hoverNumberLiteralImage,
      alt: { zh: '字面量 Hover 信息截图', en: 'Number literal hover screenshot' },
    },
    { src: inlayHintsImage, alt: { zh: 'Inlay Hints 截图', en: 'Inlay Hints screenshot' } },
  ],
  completion: [
    {
      src: completionModuleDeclImage,
      alt: { zh: '模块声明补全截图', en: 'Module declaration completion screenshot' },
    },
    { src: completionPortsImage, alt: { zh: '端口补全截图', en: 'Port completion screenshot' } },
    { src: completionItemsImage, alt: { zh: '补全候选列表截图', en: 'Completion item list screenshot' } },
    {
      src: completionSnippetModuleImage,
      alt: { zh: '模块代码片段补全截图', en: 'Module snippet completion screenshot' },
    },
    {
      src: completionModuleSnippetExpandedImage,
      alt: { zh: '展开后的模块代码片段补全截图', en: 'Expanded module snippet screenshot' },
    },
  ],
  refactoring: [
    {
      src: missingPortsImage,
      alt: { zh: '补全缺失端口 Code Action 截图', en: 'Missing-port code action screenshot' },
    },
    { src: renameImage, alt: { zh: '重命名符号截图', en: 'Symbol rename screenshot' } },
  ],
  diagnostics: [
    {
      src: diagnosticsUndeclaredIdentifiersImage,
      alt: { zh: '未定义标识符诊断截图', en: 'Undeclared identifier diagnostic screenshot' },
    },
    {
      src: diagnosticsLoopAnalysisImage,
      alt: { zh: '组合环路诊断截图', en: 'Combinational loop diagnostic screenshot' },
    },
  ],
} as const;

const localizedImages = (
  key: keyof typeof featureImages,
  locale: HomepageLocale,
): HomepageFeatureImage[] => featureImages[key].map((image) => ({ src: image.src, alt: image.alt[locale] }));

export const getHomepageFeatures = (localeInput?: string): HomepageFeature[] => {
  const locale = normalizeHomepageLocale(localeInput);

  if (locale === 'en') {
    return [
      {
        layout: 'image-left',
        eyebrow: 'Navigation',
        title: 'Symbol Navigation',
        description: `Use ${accent('Go to Definition')}, ${accent('Find References')}, and ${accent('Document Symbols')} in Vide to move quickly across modules, ports, and registers, so you can trace RTL connections without leaving the current context.<br /><br />Writing RTL no longer has to start with Ctrl + F.`,
        images: localizedImages('navigation', locale),
      },
      {
        layout: 'image-right',
        eyebrow: 'Insight',
        title: 'Code Insight',
        description: `Use Vide's ${accent('Hover')} and ${accent('Inlay Hints')} to inspect modules, literals, and port connections in one editor window, with less window switching and more focus on the RTL design itself.`,
        images: localizedImages('insight', locale),
      },
      {
        layout: 'image-left',
        eyebrow: 'Completion',
        title: 'Precise Completion',
        description: `Vide's ${accent('Completion')} understands the current code context, suggests candidates suited to instantiations, port connections, and other editing positions, and provides structured edits with ${accent('Snippets')}.`,
        images: localizedImages('completion', locale),
      },
      {
        layout: 'image-right',
        eyebrow: 'Refactoring',
        title: 'Automatic Refactoring',
        description: `With ${accent('Automatic Refactoring')} and ${accent('Rename')}, Vide handles repetitive details such as port wiring, signal renames, and literal-base conversion, taking the busywork out of RTL refactoring.`,
        images: localizedImages('refactoring', locale),
      },
      {
        layout: 'image-left',
        eyebrow: 'Diagnostics',
        title: 'Diagnostics',
        description: `Vide reports code diagnostics as you edit, so errors surface earlier.<br /><br />It can also combine with ${externalLink('https://qihe.pascal-lab.net', 'Qihe')} for deeper static analysis results directly inside the editor, helping you find potential issues.`,
        images: localizedImages('diagnostics', locale),
      },
    ];
  }

  return [
    {
      layout: 'image-left',
      eyebrow: 'Navigation',
      title: '符号导航',
      description: `在 Vide 中使用${accent('定义跳转')}、${accent('引用搜索')}和${accent('符号大纲')}在模块、端口和寄存器之间快速定位，让开发者不用离开当前上下文也能追清 RTL 连接关系。<br /><br />写 RTL，不再只能依靠 Ctrl + F。`,
      images: localizedImages('navigation', locale),
    },
    {
      layout: 'image-right',
      eyebrow: 'Insight',
      title: '代码理解',
      description: `利用 Vide 的${accent('悬停信息')}和${accent('代码注解')}在一个窗口中实时查看模块、字面量与端口连接信息，减少窗口切换的负担，让开发者更专注于 RTL 设计本身。`,
      images: localizedImages('insight', locale),
    },
    {
      layout: 'image-left',
      eyebrow: 'Completion',
      title: '精准补全',
      description: `Vide 的${accent('补全')}机制理解当前的代码上下文，能在实例化、端口连接和其他的编辑位置给出更贴近工程语义的建议，更能通过${accent('代码片段')}提供结构化补全。`,
      images: localizedImages('completion', locale),
    },
    {
      layout: 'image-right',
      eyebrow: 'Refactoring',
      title: '自动重构',
      description: `通过${accent('自动重构')}和${accent('重命名')}，把端口连线、信号重命名、转换进制这些繁琐的细节交给 Vide 完成，解放开发者的重构体验。`,
      images: localizedImages('refactoring', locale),
    },
    {
      layout: 'image-left',
      eyebrow: 'Diagnostics',
      title: '诊断分析',
      description: `Vide 能在编辑过程中实时给出代码诊断，让错误更早被发现。<br /><br />此外，Vide 能够结合${externalLink('https://qihe.pascal-lab.net', '骑河')}提供的强大静态分析能力，在编辑器中给出更深入的分析结果，帮助开发者发现潜在问题。`,
      images: localizedImages('diagnostics', locale),
    },
  ];
};

export const homepageFeatures = getHomepageFeatures('zh');

const comparisonColumnSpecs = [
  { key: 'definition', slug: 'navigation', zh: '定义跳转', en: 'Go to Definition' },
  { key: 'references', slug: 'references', zh: '引用搜索', en: 'Find References' },
  { key: 'hover', slug: 'hover', zh: '悬停信息', en: 'Hover' },
  { key: 'completion', slug: 'completion', zh: '代码补全', en: 'Completion' },
  { key: 'rename', slug: 'rename', zh: '重命名', en: 'Rename' },
  { key: 'syntaxHighlighting', slug: 'syntax-highlighting', zh: '语法高亮', en: 'Syntax Highlighting' },
  { key: 'semanticHighlighting', slug: 'semantic-highlighting', zh: '语义高亮', en: 'Semantic Highlighting' },
  { key: 'annotations', slug: 'annotations', zh: '代码注解', en: 'Annotations' },
  { key: 'documentSymbols', slug: 'document-symbols', zh: '符号大纲', en: 'Document Symbols' },
  { key: 'folding', slug: 'folding', zh: '折叠', en: 'Folding' },
  { key: 'codeActions', slug: 'quick-fixes', zh: '自动重构', en: 'Automatic Refactoring' },
  { key: 'diagnostics', slug: 'diagnostics', zh: '实时诊断', en: 'Diagnostics' },
  { key: 'signatureHelp', slug: 'signature-help', zh: '签名提示', en: 'Signature Help' },
  { key: 'selectionRange', slug: 'selection-range', zh: '语义选区', en: 'Selection Range' },
] as const;

export type ComparisonFeatureKey = (typeof comparisonColumnSpecs)[number]['key'];

const getComparisonColumns = (localeInput?: string) => {
  const locale = normalizeHomepageLocale(localeInput);
  return comparisonColumnSpecs.map((column) => ({
    key: column.key,
    label: column[locale],
    href: docsLink(column.slug),
  })) as readonly (ComparisonColumn & { key: ComparisonFeatureKey })[];
};

export const comparisonColumns = getComparisonColumns('zh');

const comparisonProductFeatures = (locale: HomepageLocale): ComparisonProduct[] => [
  {
    name: 'Quartus',
    meta: 'Intel',
    features: {
      syntaxHighlighting: true,
      definition: locale === 'en' ? 'Only supports jump from an instance to a module definition' : '仅支持从实例跳到模块定义',
      references: false,
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: locale === 'en' ? 'Only supports modules' : '仅支持模块',
      folding: true,
      selectionRange: false,
      codeActions: false,
      annotations: false,
      diagnostics: false,
    },
  },
  {
    name: 'Vivado',
    meta: 'Xilinx',
    features: {
      syntaxHighlighting: true,
      definition: true,
      references: true,
      hover: locale === 'en' ? 'Only shows variable types' : '支持显示变量的类型信息',
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: false,
      folding: true,
      selectionRange: false,
      codeActions: false,
      annotations: false,
      diagnostics: true,
    },
  },
  {
    name: 'Verible',
    meta: 'Most-Starred OSS',
    features: {
      syntaxHighlighting: locale == 'en' ? 'Does not support itself, requires editor-provided syntax highlighting' : '自身不支持，需要编辑器提供语法高亮',
      definition: true,
      references: true,
      hover: false,
      completion: false,
      rename: true,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: true,
      folding: locale == 'en' ? 'Does not support itself, requires editor-provided folding' : '自身不支持，需要编辑器提供缩进折叠',
      selectionRange: false,
      codeActions:
        locale === 'en' ? 'Only supports linter quick fixes and autoexpand' : '仅支持 linter 的 quickfix 和 autoexpand',
      annotations: false,
      diagnostics: locale === 'en' ? 'Only supports syntax errors and linter rules' : '仅支持语法错误和 linter 规则',
    },
  },
  {
    name: 'Vide',
    meta: locale === 'en' ? 'PASCAL' : 'Ours',
    highlighted: true,
    betaFeatureKeys: ['diagnostics'],
    features: {
      syntaxHighlighting: true,
      definition: true,
      references: true,
      hover: true,
      completion: true,
      rename: true,
      semanticHighlighting: true,
      signatureHelp: true,
      documentSymbols: true,
      folding: true,
      selectionRange: true,
      codeActions: true,
      annotations: true,
      diagnostics: true,
    },
  },
];

export const comparisonProducts = comparisonProductFeatures('zh');

export const getHomepageComparison = (localeInput?: string) => ({
  columns: getComparisonColumns(localeInput),
  products: comparisonProductFeatures(normalizeHomepageLocale(localeInput)),
});

export const homepageComparison = getHomepageComparison('zh');

export const getHomepageCtaActions = (localeInput?: string) => {
  const locale = normalizeHomepageLocale(localeInput);
  return [
    {
      href: './user-guide/',
      label: locale === 'en' ? 'Quick Start' : '快速开始',
      variant: 'primary',
      icon: 'right-arrow',
    },
    {
      href: locale === 'en' ? './user-guide/online-experience/' : './playground/',
      label: locale === 'en' ? 'Online Experience' : '在线体验',
      variant: 'secondary',
      icon: 'rocket',
    },
  ] as const;
};

export const homepageCtaActions = getHomepageCtaActions('zh');
