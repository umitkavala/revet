import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Revet',
  tagline: 'Code review that understands your architecture',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://umitkavala.github.io',
  baseUrl: '/revet/',

  organizationName: 'umitkavala',
  projectName: 'revet',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/umitkavala/revet/tree/main/docs/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Revet',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://github.com/umitkavala/revet',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {label: 'Getting Started', to: '/docs/getting-started'},
            {label: 'Commands', to: '/docs/commands'},
            {label: 'Configuration', to: '/docs/configuration'},
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/umitkavala/revet',
            },
            {
              label: 'Issues',
              href: 'https://github.com/umitkavala/revet/issues',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Revet. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['bash', 'toml', 'yaml', 'rust'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
