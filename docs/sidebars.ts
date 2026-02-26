import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'getting-started',
    'commands',
    {
      type: 'category',
      label: 'Analyzers',
      items: [
        'analyzers/overview',
        'analyzers/security',
        'analyzers/ml-pipeline',
        'analyzers/infrastructure',
        'analyzers/react-hooks',
        'analyzers/async-patterns',
        'analyzers/dependency',
        'analyzers/error-handling',
        'analyzers/toolchain',
        'analyzers/custom-rules',
      ],
    },
    'language-parsers',
    'output-formats',
    'configuration',
    'ci-cd',
    'architecture',
    'contributing',
  ],
};

export default sidebars;
