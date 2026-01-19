import { Link } from 'react-router-dom';

export function Homepage() {
  return (
    <div className="homepage">
      <header className="hero">
        <div className="logo">&#x1F69C;</div>
        <h1>Tractor</h1>
        <p className="tagline"><code>grep</code> for code structure, not text</p>

        <div className="install-cmd">
          cargo install tractor
        </div>

        <div className="hero-actions">
          <Link to="/playground" className="btn btn-primary">
            Try the Playground
          </Link>
          <a href="https://github.com/boukeversteegh/tractor" className="btn btn-secondary">
            GitHub
          </a>
        </div>
      </header>

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

      <section className="quick-example">
        <h2>Quick Example</h2>
        <pre><code><span className="comment"># Find async void methods (a common bug pattern)</span>{'\n'}tractor src/**/*.cs -x <span className="string">"method[async][type='void']"</span></code></pre>
      </section>

      <footer className="homepage-footer">
        <p>
          Made with &#x1F69C; by the Tractor team
        </p>
        <p>
          <a href="https://github.com/boukeversteegh/tractor">GitHub</a> &middot;
          <a href="https://github.com/boukeversteegh/tractor/issues">Issues</a>
        </p>
      </footer>
    </div>
  );
}
