import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import styles from './index.module.css';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link className="button button--secondary button--lg" to="/docs/getting-started">
            Get Started →
          </Link>
          <Link
            className="button button--outline button--secondary button--lg"
            href="https://github.com/umitkavala/revet"
            style={{marginLeft: '1rem'}}>
            GitHub
          </Link>
        </div>
      </div>
    </header>
  );
}

const features = [
  {
    title: 'Not a GPT wrapper',
    description: '80% of checks are deterministic — free, fast, and reproducible. LLM reasoning is opt-in.',
  },
  {
    title: 'Cross-file impact analysis',
    description: 'Detects breaking changes that affect other parts of your codebase, not just the changed file.',
  },
  {
    title: 'Incremental by default',
    description: 'Per-file graph cache means second runs are near-instant even on large repos.',
  },
  {
    title: '11 languages',
    description: 'Python, TypeScript, Rust, Go, Java, C#, Kotlin, Ruby, PHP, Swift, and C/C++.',
  },
  {
    title: 'Pluggable analyzers',
    description: 'Security, ML, infra, React, async, dependency, error handling, toolchain — and custom regex rules.',
  },
  {
    title: 'CI-native',
    description: 'SARIF, GitHub annotations, and inline PR review comments. Integrates in minutes.',
  },
];

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout title={siteConfig.title} description={siteConfig.tagline}>
      <HomepageHeader />
      <main>
        <section className={styles.features}>
          <div className="container">
            <div className="row">
              {features.map(({title, description}) => (
                <div key={title} className={clsx('col col--4', styles.feature)}>
                  <h3>{title}</h3>
                  <p>{description}</p>
                </div>
              ))}
            </div>
          </div>
        </section>
        <section style={{padding: '2rem 0', background: 'var(--ifm-color-emphasis-100)'}}>
          <div className="container" style={{textAlign: 'center'}}>
            <pre style={{display: 'inline-block', textAlign: 'left'}}>
              <code>{`cargo install revet
revet init
revet review`}</code>
            </pre>
          </div>
        </section>
      </main>
    </Layout>
  );
}
