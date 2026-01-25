import { useState } from 'react';

type Result = {
  line: number;
  code: string;
};

type Query = {
  label: string;
  xpath: string;
  description: string;
  highlights: string[];
  results: Result[];
};

const SOURCE_CODE = `public class UserService
{
    public async Task<User> GetAsync(int id)
    {
        return await _db.FindAsync(id);
    }

    public async Task SaveAsync(User user)
    {
        _db.Save(user); // missing await!
    }

    public void Delete(int id)
    {
        _db.Delete(id);
    }
}`;

const QUERIES: Query[] = [
  {
    label: 'method',
    xpath: 'method',
    description: 'Find all methods',
    highlights: ['get', 'save', 'delete'],
    results: [
      { line: 3, code: 'public async Task<User> GetAsync(int id) { ... }' },
      { line: 8, code: 'public async Task SaveAsync(User user) { ... }' },
      { line: 13, code: 'public void Delete(int id) { ... }' },
    ],
  },
  {
    label: 'method[async]',
    xpath: 'method[async]',
    description: 'Find async methods',
    highlights: ['get', 'save'],
    results: [
      { line: 3, code: 'public async Task<User> GetAsync(int id) { ... }' },
      { line: 8, code: 'public async Task SaveAsync(User user) { ... }' },
    ],
  },
  {
    label: 'method/name',
    xpath: 'method/name',
    description: 'Extract method names',
    highlights: ['get-name', 'save-name', 'delete-name'],
    results: [
      { line: 3, code: 'GetAsync' },
      { line: 8, code: 'SaveAsync' },
      { line: 13, code: 'Delete' },
    ],
  },
  {
    label: 'method[async][not(.//await)]',
    xpath: 'method[async][not(.//await)]',
    description: 'Detect async methods missing await',
    highlights: ['save'],
    results: [
      { line: 8, code: 'public async Task SaveAsync(User user) { ... }' },
    ],
  },
];

export function MiniPlayground() {
  const [activeQuery, setActiveQuery] = useState<Query>(QUERIES[0]); // Start with "method" (simplest)

  const highlightSource = (source: string, highlights: string[]) => {
    let result = source;

    if (highlights.includes('get')) {
      result = result.replace(
        /(public async Task<User> GetAsync.*?\n    \})/s,
        '<span class="mini-highlight">$1</span>'
      );
    }
    if (highlights.includes('save')) {
      result = result.replace(
        /(public async Task SaveAsync.*?\n    \})/s,
        '<span class="mini-highlight">$1</span>'
      );
    }
    if (highlights.includes('delete')) {
      result = result.replace(
        /(public void Delete.*?\n    \})/s,
        '<span class="mini-highlight">$1</span>'
      );
    }
    if (highlights.includes('get-name')) {
      result = result.replace(
        /(GetAsync)/,
        '<span class="mini-highlight">$1</span>'
      );
    }
    if (highlights.includes('save-name')) {
      result = result.replace(
        /(SaveAsync)/,
        '<span class="mini-highlight">$1</span>'
      );
    }
    if (highlights.includes('delete-name')) {
      result = result.replace(
        /(Delete)/,
        '<span class="mini-highlight">$1</span>'
      );
    }

    return result;
  };

  return (
    <div className="mini-playground">
      <div className="mini-title-bar">
        <span className="mini-title">{activeQuery.description}</span>
        <span className="mini-demo-label">Demo</span>
      </div>

      <div className="mini-query-row">
        <code className="mini-query">{activeQuery.xpath}</code>
        <div className="mini-queries">
          {QUERIES.map((q) => (
            <button
              key={q.xpath}
              className={`mini-query-btn ${activeQuery.xpath === q.xpath ? 'active' : ''}`}
              onClick={() => setActiveQuery(q)}
            >
              {q.label}
            </button>
          ))}
        </div>
      </div>

      <div className="mini-panels">
        <div className="mini-panel">
          <div className="mini-panel-header">
            <span>Source</span>
            <span className="mini-lang">C#</span>
          </div>
          <pre><code dangerouslySetInnerHTML={{
            __html: highlightSource(SOURCE_CODE, activeQuery.highlights)
          }} /></pre>
        </div>
        <div className="mini-panel">
          <div className="mini-panel-header">
            <span>Results</span>
            <span className="mini-matches">{activeQuery.results.length} match{activeQuery.results.length !== 1 ? 'es' : ''}</span>
          </div>
          <div className="mini-results">
            {activeQuery.results.map((r, i) => (
              <div key={i} className="mini-result">
                <span className="mini-result-line">:{r.line}</span>
                <code>{r.code}</code>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
