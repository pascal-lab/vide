import type { ImageMetadata } from 'astro';

import completionImage from '../assets/homepage-features/completion1.png';
import gotoDefImage from '../assets/homepage-features/goto-def.png';
import hoverInstanceImage from '../assets/homepage-features/hover-on-instance.png';
import hoverLiteralImage from '../assets/homepage-features/hover-on-literal.png';
import hoverModuleImage from '../assets/homepage-features/hover-on-module.png';
import inlayImage from '../assets/homepage-features/inlay.png';
import missingPortImage from '../assets/homepage-features/missing-port.png';
import peekDefImage from '../assets/homepage-features/peek-def.png';
import renameImage from '../assets/homepage-features/rename.png';

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

export type ComparisonFeatureValue = boolean | 'partial';

const accent = (text: string) =>
  `<span class="vide-feature-carousel__description-accent">${text}</span>`;

export const homepageFeatures: HomepageFeature[] = [
  {
    layout: 'image-left',
    eyebrow: 'Navigation',
    title: '符号导航',
    description: `在 Vide 中使用${accent('定义跳转')}和${accent('引用搜索')}在模块、实例和端口之间快速定位，让开发者不用离开当前上下文也能追清 RTL 连接关系。<br /><br />写 RTL，不再只能依靠 Ctrl + F。`,
    images: [
      { src: gotoDefImage, alt: 'Go to Definition 截图' },
      { src: peekDefImage, alt: 'Peek Definition 截图' },
    ],
  },
  {
    layout: 'image-right',
    eyebrow: 'Insight',
    title: '代码理解',
    description: `利用 Vide 的${accent('悬停信息')}和${accent('代码注解')}在一个窗口中实时查看模块、字面量与端口连接信息，减少窗口切换的负担，让开发者更专注于 RTL 设计本身。`,
    images: [
      { src: hoverModuleImage, alt: '模块 Hover 信息截图' },
      { src: hoverInstanceImage, alt: '例化 Hover 信息截图' },
      { src: hoverLiteralImage, alt: '字面量 Hover 信息截图' },
      { src: inlayImage, alt: 'Inlay Hints 截图' },
    ],
  },
  {
    layout: 'image-left',
    eyebrow: 'Completion',
    title: '精准补全',
    description: `Vide 的${accent('补全')}机制理解当前的代码上下文，能在实例化、端口连接和其他的编辑位置给出更贴近工程语义的建议，更能通过${accent('代码片段')}提供结构化补全。`,
    images: [{ src: completionImage, alt: '模块和端口补全截图' }],
  },
  {
    layout: 'image-right',
    eyebrow: 'Refactoring',
    title: '自动重构',
    description: `通过${accent('自动重构')}和${accent('重命名')}，把端口连线、信号重命名、转换进制这些繁琐的细节交给 Vide 完成，解放开发者的重构体验。`,
    images: [
      { src: missingPortImage, alt: '补全缺失端口 Code Action 截图' },
      { src: renameImage, alt: '重命名符号截图' },
    ],
  },
];

export const comparisonColumns = [
  { key: 'definition', label: '定义跳转' },
  { key: 'references', label: '引用搜索' },
  { key: 'hover', label: '悬停信息' },
  { key: 'completion', label: '代码补全' },
  { key: 'rename', label: '重命名' },
  { key: 'syntaxHighlighting', label: '语法高亮' },
  { key: 'semanticHighlighting', label: '语义高亮' },
  { key: 'inlayHints', label: '代码注解' },
  { key: 'documentSymbols', label: '符号大纲' },
  { key: 'folding', label: '折叠' },
  { key: 'codeActions', label: '自动重构' },
  { key: 'diagnostics', label: '实时诊断' },
  { key: 'signatureHelp', label: '签名提示' },
  { key: 'selectionRange', label: '语义选区' },
] as const;

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
      definition: false,
      references: false,
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: 'partial',
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
      definition: false,
      references: false,
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: false,
      folding: false,
      selectionRange: false,
      codeActions: false,
      inlayHints: false,
      diagnostics: false,
    },
  },
  {
    name: 'Verible',
    meta: 'Most-Starred OSS',
    features: {
      syntaxHighlighting: true,
      definition: 'partial',
      references: 'partial',
      hover: false,
      completion: false,
      rename: false,
      semanticHighlighting: false,
      signatureHelp: false,
      documentSymbols: true,
      folding: false,
      selectionRange: false,
      codeActions: 'partial',
      inlayHints: false,
      diagnostics: true,
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
