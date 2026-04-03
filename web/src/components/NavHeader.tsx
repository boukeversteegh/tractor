import { Link, useLocation } from 'react-router-dom';

export function NavHeader() {
  const location = useLocation();
  const isActive = (path: string) => {
    if (path === '/docs') return location.pathname.startsWith('/docs');
    return location.pathname === path;
  };

  return (
    <nav className="nav-header">
      <Link to="/" className="nav-brand">
        <span className="nav-logo">&#x1F69C;</span>
        <span className="nav-name">Tractor</span>
      </Link>
      <div className="nav-links">
        <Link to="/docs" className={`nav-link ${isActive('/docs') ? 'active' : ''}`}>
          Docs
        </Link>
        <Link to="/playground" className={`nav-link ${isActive('/playground') ? 'active' : ''}`}>
          Playground
        </Link>
        <a
          href="https://github.com/boukeversteegh/tractor"
          className="nav-link nav-link-external"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
            <polyline points="15 3 21 3 21 9" />
            <line x1="10" y1="14" x2="21" y2="3" />
          </svg>
        </a>
      </div>
    </nav>
  );
}
