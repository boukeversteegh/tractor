import { Match } from '../xpath';

interface QueryResultsProps {
  matches: Match[];
}

export function QueryResults({ matches }: QueryResultsProps) {
  if (matches.length === 0) {
    return (
      <div className="query-results empty">
        No matches. Try a different query.
      </div>
    );
  }

  return (
    <div className="query-results">
      {matches.slice(0, 50).map((match, i) => (
        <div key={i} className="match">
          {match.start && (
            <span className="match-location">
              {match.start} - {match.end}
            </span>
          )}
          <pre><code>{match.xml.slice(0, 200)}{match.xml.length > 200 ? '...' : ''}</code></pre>
        </div>
      ))}
      {matches.length > 50 && (
        <div className="more-matches">
          ...and {matches.length - 50} more matches
        </div>
      )}
    </div>
  );
}
