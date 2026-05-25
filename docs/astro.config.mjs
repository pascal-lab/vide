import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const base = process.env.ASTRO_BASE ?? '/';

export default defineConfig({
  site: 'https://pascal-lab.github.io',
  base,
  integrations: [
    starlight({
      title: {
        'zh-CN': 'Vizsla 用户手册',
        en: 'Vizsla User Guide',
      },
      description:
        'User guide for the Vizsla Verilog/SystemVerilog language server and VS Code extension.',
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
      sidebar: [
        'quick-start',
        'installation',
        'first-project',
        'project-configuration',
        'parsing-and-analysis',
        'daily-use',
        'vscode-settings',
        'commands-status-logs',
        'check-server',
        'build-from-source',
        'troubleshooting',
        {
          label: '更新日志',
          translations: { en: 'Changelog' },
          items: ['changelog', 'changelog/v0-1-1', 'changelog/v0-1-2', 'changelog/v0-1-3', 'changelog/v0-1-4'],
        },
      ],
    }),
  ],
});
