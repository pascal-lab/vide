import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightUtils from '@lorenzo_lewis/starlight-utils';

const base = process.env.ASTRO_BASE ?? '/';
const site = process.env.ASTRO_SITE ?? 'https://vide.pascal-lab.net';

export default defineConfig({
  site,
  base,
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
        baseUrl: 'https://github.com/pascal-lab/vide/edit/master/docs/',
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
            'user-guide/vscode-installation',
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
              items: ['advanced-guide/advanced-installation', 'advanced-guide/build-from-source'],
            },
            {
              label: '排障与日志',
              translations: { en: 'Troubleshooting and Logs' },
              items: [
                'advanced-guide/check-server',
                'advanced-guide/troubleshooting',
              ],
            },
            {
              label: '分析模型',
              translations: { en: 'Analysis Model' },
              items: ['advanced-guide/parsing-and-analysis'],
            },
          ],
        },
        {
          label: 'Changelog',
          translations: { en: 'Changelog' },
          items: ['changelog'],
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
