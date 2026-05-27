import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const base = process.env.ASTRO_BASE ?? '/';
const site = process.env.ASTRO_SITE ?? 'https://vide.pascal-lab.net';

export default defineConfig({
  site,
  base,
  integrations: [
    starlight({
      title: {
        'zh-CN': 'VIDE',
        en: 'Vide Docs',
      },
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
        baseUrl: 'https://github.com/pascal-lab/vizsla/edit/master/docs/',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/pascal-lab/vizsla',
        },
      ],
      customCss: ['./src/assets/landing.css'],
      components: {
        Header: './src/components/Header.astro',
      },
      routeMiddleware: './src/starlightRouteData.ts',
      sidebar: [
        {
          label: '用户手册',
          translations: { en: 'User Guide' },
          items: [
            'user-guide',
            'user-guide/installation',
            'user-guide/quick-start',
            'user-guide/first-project',
            'user-guide/project-configuration',
            {
              label: '日常使用',
              translations: { en: 'Daily Use' },
              items: [
                'user-guide/daily-use',
                'user-guide/daily-use/language-support',
                'user-guide/daily-use/diagnostics',
                'user-guide/daily-use/navigation',
                'user-guide/daily-use/editing-assistance',
                'user-guide/daily-use/formatting',
                'user-guide/daily-use/structure',
                'user-guide/daily-use/qihe',
              ],
            },
          ],
        },
        {
          label: '进阶',
          translations: { en: 'Advanced' },
          items: [
            'advanced-guide',
            'advanced-guide/advanced-installation',
            'advanced-guide/build-from-source',
            'advanced-guide/check-server',
            'advanced-guide/commands-status-logs',
            'advanced-guide/troubleshooting',
            'advanced-guide/parsing-and-analysis',
            'advanced-guide/vscode-settings',
          ],
        },
        {
          label: 'Changelog',
          translations: { en: 'Changelog' },
          items: [
            'changelog',
            'changelog/v0-1-5',
            'changelog/v0-1-4',
            'changelog/v0-1-3',
            'changelog/v0-1-2',
            'changelog/v0-1-1',
          ],
        },
        {
          label: 'Playground',
          translations: { en: 'Playground' },
          items: ['playground'],
        },
      ],
    }),
  ],
});
