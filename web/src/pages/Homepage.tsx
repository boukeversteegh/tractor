import { useState } from 'react';
import { Link } from 'react-router-dom';
import { MiniPlayground } from '../components/MiniPlayground';

type Platform = 'linux' | 'macos' | 'windows';

const RELEASE_BASE = 'https://github.com/boukeversteegh/tractor/releases/latest/download';

const DOWNLOADS: Record<Platform, { url: string; filename: string }> = {
  linux: { url: `${RELEASE_BASE}/tractor-linux-x86_64`, filename: 'tractor-linux-x86_64' },
  macos: { url: `${RELEASE_BASE}/tractor-macos-arm64`, filename: 'tractor-macos-arm64' },
  windows: { url: `${RELEASE_BASE}/tractor-windows-x86_64-setup.exe`, filename: 'tractor-windows-x86_64-setup.exe' },
};

function detectPlatform(): Platform {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('win')) return 'windows';
  if (ua.includes('mac')) return 'macos';
  return 'linux';
}

export function Homepage() {
  const [platform, setPlatform] = useState<Platform>(detectPlatform);

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
            <span>Install</span>
            <div className="platform-switch">
              <button
                className={`platform-btn ${platform === 'linux' ? 'active' : ''}`}
                onClick={() => setPlatform('linux')}
              >
                Linux
              </button>
              <button
                className={`platform-btn ${platform === 'macos' ? 'active' : ''}`}
                onClick={() => setPlatform('macos')}
              >
                macOS
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
              <span className="step-label">1. Download</span>
              <pre className="install-cmd"><code><a href={DOWNLOADS[platform].url} className="download-link">{DOWNLOADS[platform].filename}</a></code></pre>
            </div>
            {platform === 'linux' && (
              <div className="install-step">
                <span className="step-label">2. Install</span>
                <pre className="install-cmd"><code>chmod +x tractor-linux-x86_64{'\n'}sudo mv tractor-linux-x86_64 /usr/local/bin/tractor</code></pre>
              </div>
            )}
            {platform === 'macos' && (
              <div className="install-step">
                <span className="step-label">2. Install</span>
                <pre className="install-cmd"><code>xattr -d com.apple.quarantine tractor-macos-arm64{'\n'}chmod +x tractor-macos-arm64{'\n'}sudo mv tractor-macos-arm64 /usr/local/bin/tractor</code></pre>
              </div>
            )}
            {platform === 'windows' && (
              <div className="install-step">
                <span className="step-label">2. Install</span>
                <pre className="install-cmd"><code>.\tractor-windows-x86_64-setup.exe</code></pre>
              </div>
            )}
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
