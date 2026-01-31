import { useState } from 'react';
import { Link } from 'react-router-dom';
import { MiniPlayground } from '../components/MiniPlayground';

type Platform = 'unix' | 'windows';

export function Homepage() {
  const [platform, setPlatform] = useState<Platform>('unix');

  return (
    <div className="homepage">
      <header className="hero">
        <h1><span className="logo">&#x1F69C;&#x1F4A8;</span> Tractor</h1>
        <p className="tagline">Extract patterns from your code</p>
        <ul className="use-cases">
          <li>Find code patterns</li>
          <li>Build custom linters</li>
          <li>Enforce conventions</li>
        </ul>
      </header>

      <section className="demo-section">
        <MiniPlayground />
        <div className="demo-actions">
          <Link to="/playground" className="btn btn-primary">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            Try the Playground
          </Link>
          <a href="https://github.com/boukeversteegh/tractor" className="btn btn-secondary">
            GitHub
          </a>
        </div>
      </section>

      <section className="install-section-wrapper">
        <div className="install-section">
          <div className="install-header">
            <span>Setup</span>
            <div className="platform-switch">
              <button
                className={`platform-btn ${platform === 'unix' ? 'active' : ''}`}
                onClick={() => setPlatform('unix')}
              >
                Linux / macOS
              </button>
              <button
                className={`platform-btn ${platform === 'windows' ? 'active' : ''}`}
                onClick={() => setPlatform('windows')}
              >
                Windows
              </button>
            </div>
          </div>

          <div className="install-steps">
            <div className="install-step">
              <span className="step-label">1. Install Rust</span>
              {platform === 'unix' ? (
                <pre className="install-cmd"><code>curl -fsSL https://sh.rustup.rs | sh</code></pre>
              ) : (
                <pre className="install-cmd"><code>winget install Rustlang.Rustup</code></pre>
              )}
            </div>
            <div className="install-step">
              <span className="step-label">2. Install Tractor</span>
              <pre className="install-cmd"><code>cargo install --git https://github.com/boukeversteegh/tractor tractor</code></pre>
            </div>
          </div>
        </div>
      </section>

      <section className="features">
        <div className="feature-grid">
          <div className="feature">
            <h3>Transparent</h3>
            <p>Run <code>tractor file.cs</code> to see exactly what you're querying. No hidden structure.</p>
          </div>
          <div className="feature">
            <h3>Standard</h3>
            <p>XPath is a W3C standard. Millions know it. Stack Overflow has 50k+ answers.</p>
          </div>
          <div className="feature">
            <h3>Readable</h3>
            <p><code>//method[public][async]</code> reads like English. No DSL to memorize.</p>
          </div>
          <div className="feature">
            <h3>CI Ready</h3>
            <p>Ban patterns, require conventions, ensure coverage. Fail builds on violations.</p>
          </div>
        </div>
      </section>

      <footer className="homepage-footer">
        <p>
          Made with &#x1F69C; by the Tractor team
        </p>
        <p>
          <a href="https://github.com/boukeversteegh/tractor">GitHub</a> &middot;
          <a href="https://github.com/boukeversteegh/tractor/issues">Issues</a> &middot;
          <a href="/versions.json">Versions</a>
        </p>
      </footer>
    </div>
  );
}
