import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://pascal-lab.github.io',
  base: '/vizsla',
  integrations: [
    starlight({
      title: 'Vizsla 用户手册',
      description: 'Vizsla Verilog/SystemVerilog 语言服务器和 VS Code 扩展用户手册。',
      locales: {
        root: {
          label: '简体中文',
          lang: 'zh-CN',
        },
      },
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
        'daily-use',
        'vscode-settings',
        'commands-status-logs',
        'check-server',
        'build-from-source',
        'troubleshooting',
        {
          label: 'Changelog',
          items: ['changelog', 'changelog/v0-1-1', 'changelog/v0-1-2'],
        },
      ],
    }),
  ],
});
