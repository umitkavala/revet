import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'getting-started',
    {
      type: 'category',
      label: 'Commands',
      items: [
        'commands/overview',
        'commands/review',
        'commands/diff',
        'commands/baseline',
        'commands/log',
        'commands/watch',
        'commands/init',
        'commands/explain',
      ],
    },
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
    'ai-reasoning',
    'configuration',
    'ci-cd',
    'architecture',
    'contributing',
  ],
};

export default sidebars;
