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
        {
          label: '从这里开始',
          translations: { en: 'Start Here' },
          items: ['installation', 'quick-start', 'playground'],
        },
        {
          label: '项目设置',
          translations: { en: 'Project Setup' },
          items: ['first-project', 'project-configuration', 'parsing-and-analysis'],
        },
        {
          label: '日常使用',
          translations: { en: 'Daily Work' },
          items: ['daily-use'],
        },
        {
          label: '操作',
          translations: { en: 'Operations' },
          items: ['check-server', 'commands-status-logs', 'troubleshooting'],
        },
        {
          label: '参考',
          translations: { en: 'Reference' },
          items: ['vscode-settings', 'build-from-source'],
        },
        {
          label: '更新日志',
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
      ],
    }),
  ],
});
