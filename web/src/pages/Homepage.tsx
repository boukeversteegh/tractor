import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { MiniPlayground } from '../components/MiniPlayground';

type Platform = 'linux' | 'macos' | 'windows';

interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
}

interface GitHubRelease {
  assets?: GitHubReleaseAsset[];
}

const RELEASE_BASE = 'https://github.com/boukeversteegh/tractor/releases/latest/download';
const RELEASES_PAGE = 'https://github.com/boukeversteegh/tractor/releases/latest';
const GITHUB_API_LATEST = 'https://api.github.com/repos/boukeversteegh/tractor/releases/latest';

const STATIC_DOWNLOADS: Record<'linux' | 'macos', { url: string; filename: string }> = {
  linux: { url: `${RELEASE_BASE}/tractor-linux-x86_64`, filename: 'tractor-linux-x86_64' },
  macos: { url: `${RELEASE_BASE}/tractor-macos-arm64`, filename: 'tractor-macos-arm64' },
};

function detectPlatform(): Platform {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('win')) return 'windows';
  if (ua.includes('mac')) return 'macos';
  return 'linux';
}

export function Homepage() {
  const [platform, setPlatform] = useState<Platform>(detectPlatform);
  const [windowsInstaller, setWindowsInstaller] = useState<{ url: string; filename: string } | null>(null);

  useEffect(() => {
    fetch(GITHUB_API_LATEST)
      .then(r => r.json())
      .then((release: GitHubRelease) => {
        const asset = release.assets?.find(a => /tractor-.*-windows-x86_64-setup\.exe$/.test(a.name));
        if (asset) {
          setWindowsInstaller({ url: asset.browser_download_url, filename: asset.name });
        }
      })
      .catch(() => { /* fall back to releases page link */ });
  }, []);

  return (
    <div className="homepage">
      <header className="hero">
        <h1><span className="logo">&#x1F69C;</span> Tractor</h1>
        <p className="tagline">Write a rule once. Enforce it everywhere.</p>
        <p className="hero-description">
          Query your code structure across 20+ languages. Find patterns, enforce conventions, and catch issues — with one tool.
        </p>
        <ul className="use-cases">
          <li>Enforce conventions in CI</li>
          <li>Find structural patterns</li>
          <li>Build custom lint rules</li>
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
              {platform === 'windows' ? (
                windowsInstaller ? (
                  <pre className="install-cmd"><code><a href={windowsInstaller.url} className="download-link">{windowsInstaller.filename}</a></code></pre>
                ) : (
                  <pre className="install-cmd"><code><a href={RELEASES_PAGE} className="download-link">Latest release (GitHub)</a></code></pre>
                )
              ) : (
                <pre className="install-cmd"><code><a href={STATIC_DOWNLOADS[platform].url} className="download-link">{STATIC_DOWNLOADS[platform].filename}</a></code></pre>
              )}
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
                <pre className="install-cmd"><code>{windowsInstaller ? `.\\${windowsInstaller.filename}` : 'Run the downloaded installer'}</code></pre>
              </div>
            )}
          </div>
        </div>
      </section>

      <section className="features">
        <div className="feature-grid">
          <div className="feature">
            <h3>Transparent</h3>
            <p>Run <code>tractor file.cs</code> to see exactly what you're querying. No hidden structure, no guessing.</p>
          </div>
          <div className="feature">
            <h3>AI-Friendly</h3>
            <p>Uses standard query syntax that AI tools already know. Get working queries from ChatGPT or Claude on the first try.</p>
          </div>
          <div className="feature">
            <h3>Multi-Language</h3>
            <p>One tool, one syntax, 20+ languages. Learn it once, apply it to C#, TypeScript, Python, Rust, Go, and more.</p>
          </div>
          <div className="feature">
            <h3>CI-Native</h3>
            <p>Enforce conventions in your pipeline. GCC and GitHub Actions output, exit codes, and match expectations built in.</p>
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
