import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const isCloudflarePages = process.env.CF_PAGES === '1';
const site = process.env.ASTRO_SITE ?? process.env.CF_PAGES_URL ?? 'https://pascal-lab.github.io';
const base = process.env.ASTRO_BASE ?? (isCloudflarePages ? '/' : '/vizsla');

export default defineConfig({
  site,
  base,
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
      ],
    }),
  ],
});
