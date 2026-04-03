import { Link, useLocation } from 'react-router-dom';
import { NavHeader } from './NavHeader';

interface SidebarSection {
  title: string;
  items: { label: string; path: string }[];
}

const SIDEBAR_SECTIONS: SidebarSection[] = [
  {
    title: 'Getting Started',
    items: [
      { label: 'Overview', path: '/docs' },
    ],
  },
  {
    title: 'Commands',
    items: [
      { label: 'query', path: '/docs/commands/query' },
      { label: 'check', path: '/docs/commands/check' },
      { label: 'test', path: '/docs/commands/test' },
      { label: 'set', path: '/docs/commands/set' },
      { label: 'run', path: '/docs/commands/run' },
    ],
  },
  {
    title: 'Guides',
    items: [
      { label: 'Query Syntax', path: '/docs/guides/query-syntax' },
      { label: 'Writing Queries', path: '/docs/guides/writing-queries' },
      { label: 'Exploring with Schema', path: '/docs/guides/schema' },
      { label: 'Writing Lint Rules', path: '/docs/guides/lint-rules' },
      { label: 'CI/CD Integration', path: '/docs/guides/ci-cd' },
    ],
  },
];

interface DocLayoutProps {
  children: React.ReactNode;
}

export function DocLayout({ children }: DocLayoutProps) {
  const location = useLocation();

  return (
    <div className="doc-page">
      <NavHeader />
      <div className="doc-container">
        <aside className="doc-sidebar">
          {SIDEBAR_SECTIONS.map((section) => (
            <div key={section.title} className="sidebar-section">
              <h4 className="sidebar-heading">{section.title}</h4>
              <ul className="sidebar-list">
                {section.items.map((item) => (
                  <li key={item.path}>
                    <Link
                      to={item.path}
                      className={`sidebar-link ${location.pathname === item.path ? 'active' : ''}`}
                    >
                      {item.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </aside>
        <main className="doc-content">
          {children}
        </main>
      </div>
    </div>
  );
}
