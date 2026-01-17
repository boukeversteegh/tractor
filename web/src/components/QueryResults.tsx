import { Match, OutputFormat } from '../xpath';
import { parsePositionString } from '../xmlTree';
import { extractSourceSnippetSync, getSourceLinesSync, prettyPrintXmlSync, ansiToHtml } from '../tractor';

interface QueryResultsProps {
  matches: Match[];
  format: OutputFormat;
  source: string;
  fileName?: string;
  hoveredIndex?: number | null;
  onHoverChange?: (index: number | null) => void;
  onMatchClick?: (index: number) => void;
}

export function QueryResults({ matches, format, source, fileName = 'input', hoveredIndex, onHoverChange, onMatchClick }: QueryResultsProps) {
  if (format === 'count') {
    return (
      <div className="query-results">
        <pre><code>{matches.length}</code></pre>
      </div>
    );
  }

  if (matches.length === 0) {
    return (
      <div className="query-results empty">
        No matches. Try a different query.
      </div>
    );
  }

  if (format === 'json') {
    const jsonData = matches.map((m) => {
      const startPos = parsePositionString(m.start || '');
      return {
        file: fileName,
        line: startPos?.line || 0,
        column: startPos?.column || 0,
        value: m.value,
      };
    });
    return (
      <div className="query-results">
        <pre><code>{JSON.stringify(jsonData, null, 2)}</code></pre>
      </div>
    );
  }

  const displayMatches = matches.slice(0, 50);

  return (
    <div className="query-results">
      {displayMatches.map((match, i) => (
        <div
          key={i}
          className={`match ${hoveredIndex === i ? 'match-hovered' : ''}`}
          onMouseEnter={() => onHoverChange?.(i)}
          onMouseLeave={() => onHoverChange?.(null)}
          onClick={() => onMatchClick?.(i)}
          style={{ cursor: 'pointer' }}
        >
          {format === 'xml' && (
            <>
              {match.start && (
                <span className="match-location">
                  {match.start} - {match.end}
                </span>
              )}
              <pre><code dangerouslySetInnerHTML={{
                __html: (() => {
                  const pretty = prettyPrintXmlSync(match.xml, false, true);
                  const truncated = pretty.length > 4000
                    ? pretty.slice(0, 4000) + '\x1b[0m...'
                    : pretty;
                  return ansiToHtml(truncated);
                })()
              }} /></pre>
            </>
          )}
          {format === 'value' && (
            <pre><code>{match.value}</code></pre>
          )}
          {format === 'lines' && (
            <pre><code>{getSourceLinesSync(source, match.start, match.end).join('\n')}</code></pre>
          )}
          {format === 'source' && (
            <pre><code>{extractSourceSnippetSync(source, match.start, match.end)}</code></pre>
          )}
          {format === 'gcc' && (
            <pre><code>{fileName}:{match.start?.replace(':', ':')}: {match.value.slice(0, 50)}{match.value.length > 50 ? '...' : ''}</code></pre>
          )}
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
