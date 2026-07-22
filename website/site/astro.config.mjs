import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightUtils from '@lorenzo_lewis/starlight-utils';
import { readdirSync } from 'node:fs';

const base = process.env.ASTRO_BASE ?? '/';
const site = process.env.ASTRO_SITE ?? 'https://vide.pascal-lab.net';
const changelogItems = ['changelog', ...getChangelogVersionItems()];

function getChangelogVersionItems() {
  const changelogDir = new URL('./src/content/docs/changelog/', import.meta.url);

  return readdirSync(changelogDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && /^v\d+(?:-\d+)+$/.test(entry.name))
    .sort(compareReleaseDirsDescending)
    .map((entry) => `changelog/${entry.name}`);
}

function compareReleaseDirsDescending(left, right) {
  const leftParts = parseReleaseDir(left.name);
  const rightParts = parseReleaseDir(right.name);
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const difference = (rightParts[index] ?? 0) - (leftParts[index] ?? 0);
    if (difference !== 0) return difference;
  }

  return right.name.localeCompare(left.name);
}

function parseReleaseDir(name) {
  return name
    .slice(1)
    .split('-')
    .map((part) => Number.parseInt(part, 10));
}

export default defineConfig({
  site,
  base,
  trailingSlash: 'always',
  integrations: [
    starlight({
      title: {
        'zh-CN': 'VIDE',
        en: 'VIDE',
      },
      favicon: '/favicon.svg',
      description:
        'Documentation for the Vide Verilog/SystemVerilog language server, VS Code extension, and playground.',
      locales: {
        root: {
          label: '简体中文',
          lang: 'zh-CN',
        },
        en: {
          label: 'English',
          lang: 'en',
        },
      },
      defaultLocale: 'root',
      editLink: {
        baseUrl: 'https://github.com/pascal-lab/vide/edit/master/website/site/',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/pascal-lab/vide',
        },
      ],
      components: {
        Footer: './src/components/SiteFooter.astro',
        Hero: './src/components/Hero.astro',
        Sidebar: './src/components/Sidebar.astro',
      },
      customCss: ['./src/assets/landing.css'],
      plugins: [
        starlightUtils({
          multiSidebar: {
            switcherStyle: 'hidden',
          },
          navLinks: {
            leading: { useSidebarLabelled: 'Header' },
          },
        }),
      ],
      routeMiddleware: './src/starlightRouteData.ts',
      sidebar: [
        {
          label: '用户手册',
          translations: { en: 'User Guide' },
          items: [
            'user-guide',
            'user-guide/online-experience',
            {
              label: '安装指南',
              translations: { en: 'Installation Guide' },
              items: [
                'user-guide/installation',
                'user-guide/vscode-installation',
                'user-guide/zed-installation',
                'user-guide/neovim-installation',
                'user-guide/emacs-installation',
              ],
            },
            'user-guide/first-project',
            {
              label: '功能特性',
              translations: { en: 'Features' },
              items: [
                'user-guide/features',
                'user-guide/features/navigation',
                'user-guide/features/references',
                'user-guide/features/hover',
                'user-guide/features/completion',
                'user-guide/features/rename',
                'user-guide/features/syntax-highlighting',
                'user-guide/features/semantic-highlighting',
                'user-guide/features/annotations',
                'user-guide/features/document-symbols',
                'user-guide/features/folding',
                'user-guide/features/quick-fixes',
                'user-guide/features/diagnostics',
                'user-guide/features/signature-help',
                'user-guide/features/selection-range',
                'user-guide/features/formatting',
                'user-guide/features/qihe',
              ],
            },
            {
              label: '参考',
              translations: { en: 'Reference' },
              items: [
                'user-guide/project-configuration',
                'user-guide/project-configuration-effects',
                'user-guide/vscode-settings',
                'user-guide/commands-status-logs',
              ],
            },
          ],
        },
        {
          label: '进阶',
          translations: { en: 'Advanced' },
          items: [
            'advanced-guide',
            {
              label: '安装与构建',
              translations: { en: 'Installation and Build' },
              items: ['advanced-guide/advanced-installation'],
            },
            {
              label: '用户配置',
              translations: { en: 'User Configuration' },
              items: ['advanced-guide/user-configuration'],
            },
            {
              label: '故障报告与排查',
              translations: { en: 'Troubleshooting and Bug Reports' },
              items: ['advanced-guide/troubleshooting'],
            },
          ],
        },
        {
          label: 'Changelog',
          translations: { en: 'Changelog' },
          items: changelogItems,
        },
        {
          label: 'Playground',
          translations: { en: 'Playground' },
          items: ['playground'],
        },
        {
          label: 'Header',
          items: [
            {
              label: '用户手册',
              translations: { en: 'User Guide' },
              link: '/user-guide/',
            },
            {
              label: '进阶',
              translations: { en: 'Advanced' },
              link: '/advanced-guide/',
            },
            {
              label: '更新日志',
              translations: { en: 'Changelog' },
              link: '/changelog/',
            },
            {
              label: 'Playground',
              translations: { en: 'Playground' },
              link: '/playground/',
            },
          ],
        },
      ],
    }),
  ],
});
