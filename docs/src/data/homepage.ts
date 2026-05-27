import type { ImageMetadata } from 'astro';

import completionItemsImage from '../assets/homepage-features/completion-items.png';
import completionModuleDeclImage from '../assets/homepage-features/completion-module-decl.png';
import completionModuleSnippetExpandedImage from '../assets/homepage-features/completion-module-snippets-expanded.png';
import completionPortsImage from '../assets/homepage-features/completion-ports.png';
import completionSnippetModuleImage from '../assets/homepage-features/completion-snippets-module.png';
import documentSymbolImage from '../assets/homepage-features/document-symbol.jpeg';
import findAllReferencesImage from '../assets/homepage-features/find-all-references.jpeg';
import hoverInstanceNameImage from '../assets/homepage-features/hover-on-instance-name.png';
import hoverModuleNameImage from '../assets/homepage-features/hover-on-module-name.png';
import hoverNumberLiteralImage from '../assets/homepage-features/hover-on-number-literal.png';
import inlayHintsImage from '../assets/homepage-features/inlay-hints.png';
import missingPortsImage from '../assets/homepage-features/missing-ports.png';
import peekDefinitionImage from '../assets/homepage-features/peek-definition.png';
import renameImage from '../assets/homepage-features/rename-updated.png';

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

const accent = (text: string) =>
  `<span class="vide-feature-carousel__description-accent">${text}</span>`;

const docsLink = (slug: string) => `./user-guide/daily-use/${slug}/`;

const column = (key: string, label: string, slug: string): ComparisonColumn => ({
  key,
  label,
  href: docsLink(slug),
});

export const homepageFeatures: HomepageFeature[] = [
  {
    layout: 'image-left',
    eyebrow: 'Navigation',
    title: '符号导航',
    description: `在 Vide 中使用${accent('定义跳转')}、${accent('引用搜索')}和${accent('符号大纲')}在模块、端口和寄存器之间快速定位，让开发者不用离开当前上下文也能追清 RTL 连接关系。<br /><br />写 RTL，不再只能依靠 Ctrl + F。`,
    images: [
      { src: peekDefinitionImage, alt: 'Peek Definition 截图' },
      { src: findAllReferencesImage, alt: 'Find All References 截图' },
      { src: documentSymbolImage, alt: 'Document Symbol 截图' },
    ],
  },
  {
    layout: 'image-right',
    eyebrow: 'Insight',
    title: '代码理解',
    description: `利用 Vide 的${accent('悬停信息')}和${accent('代码注解')}在一个窗口中实时查看模块、字面量与端口连接信息，减少窗口切换的负担，让开发者更专注于 RTL 设计本身。`,
    images: [
      { src: hoverModuleNameImage, alt: '模块 Hover 信息截图' },
      { src: hoverInstanceNameImage, alt: '例化 Hover 信息截图' },
      { src: hoverNumberLiteralImage, alt: '字面量 Hover 信息截图' },
      { src: inlayHintsImage, alt: 'Inlay Hints 截图' },
    ],
  },
  {
    layout: 'image-left',
    eyebrow: 'Completion',
    title: '精准补全',
    description: `Vide 的${accent('补全')}机制理解当前的代码上下文，能在实例化、端口连接和其他的编辑位置给出更贴近工程语义的建议，更能通过${accent('代码片段')}提供结构化补全。`,
    images: [
      { src: completionModuleDeclImage, alt: '模块声明补全截图' },
      { src: completionPortsImage, alt: '端口补全截图' },
      { src: completionItemsImage, alt: '补全候选列表截图' },
      { src: completionSnippetModuleImage, alt: '模块代码片段补全截图' },
      { src: completionModuleSnippetExpandedImage, alt: '展开后的模块代码片段补全截图' },
    ],
  },
  {
    layout: 'image-right',
    eyebrow: 'Refactoring',
    title: '自动重构',
    description: `通过${accent('自动重构')}和${accent('重命名')}，把端口连线、信号重命名、转换进制这些繁琐的细节交给 Vide 完成，解放开发者的重构体验。`,
    images: [
      { src: missingPortsImage, alt: '补全缺失端口 Code Action 截图' },
      { src: renameImage, alt: '重命名符号截图' },
    ],
  },
];

export const comparisonColumns = [
  column('definition', '定义跳转', 'navigation'),
  column('references', '引用搜索', 'navigation'),
  column('hover', '悬停信息', 'navigation'),
  column('completion', '代码补全', 'editing-assistance'),
  column('rename', '重命名', 'editing-assistance'),
  column('syntaxHighlighting', '语法高亮', 'language-support'),
  column('semanticHighlighting', '语义高亮', 'structure'),
  column('inlayHints', '代码注解', 'structure'),
  column('documentSymbols', '符号大纲', 'structure'),
  column('folding', '折叠', 'structure'),
  column('codeActions', '自动重构', 'editing-assistance'),
  column('diagnostics', '实时诊断', 'diagnostics'),
  column('signatureHelp', '签名提示', 'editing-assistance'),
  column('selectionRange', '语义选区', 'structure'),
] as const satisfies readonly ComparisonColumn[];

export type ComparisonFeatureKey = (typeof comparisonColumns)[number]['key'];

export interface ComparisonProduct {
  name: string;
  meta: string;
  highlighted?: boolean;
  betaFeatureKeys?: readonly ComparisonFeatureKey[];
  features: Record<ComparisonFeatureKey, ComparisonFeatureValue>;
}

export const comparisonProducts: ComparisonProduct[] = [
  {
    name: 'Quartus',
    meta: 'Intel',
    features: {
      syntaxHighlighting: true,
      definition: '支持从实例跳到模块定义',
      references: false,
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: '支持模块',
      folding: true,
      selectionRange: false,
      codeActions: false,
      inlayHints: false,
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
      hover: '支持显示变量的类型信息',
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: false,
      folding: true,
      selectionRange: false,
      codeActions: false,
      inlayHints: false,
      diagnostics: true,
    },
  },
  {
    name: 'Verible',
    meta: 'Most-Starred OSS',
    features: {
      syntaxHighlighting: false,
      definition: true,
      references: true,
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: true,
      folding: false,
      selectionRange: false,
      codeActions: '支持 linter 的 quickfix 和 autoexpand',
      inlayHints: false,
      diagnostics: '支持语法错误和 linter 规则',
    },
  },
  {
    name: 'Vide',
    meta: 'Ours',
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
      inlayHints: true,
      diagnostics: true,
    },
  },
];

export const homepageComparison = {
  columns: comparisonColumns,
  products: comparisonProducts,
};

export const homepageCtaActions = [
  {
    href: './user-guide/quick-start/',
    label: '快速开始',
    variant: 'primary',
    icon: 'right-arrow',
  },
  {
    href: './playground/',
    label: '在线体验',
    variant: 'secondary',
    icon: 'rocket',
  },
] as const;
