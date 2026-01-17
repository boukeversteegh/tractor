import { Match, OutputFormat } from '../xpath';
import { parsePositionString, positionToOffset } from '../xmlTree';

interface QueryResultsProps {
  matches: Match[];
  format: OutputFormat;
  source: string;
  fileName?: string;
}

function getSourceLines(source: string, start?: string, end?: string): string[] {
  if (!start || !end) return [];
  const startPos = parsePositionString(start);
  const endPos = parsePositionString(end);
  if (!startPos || !endPos) return [];

  const lines = source.split('\n');
  return lines.slice(startPos.line - 1, endPos.line);
}

function getSourceSnippet(source: string, start?: string, end?: string): string {
  if (!start || !end) return '';
  const startPos = parsePositionString(start);
  const endPos = parsePositionString(end);
  if (!startPos || !endPos) return '';

  const startOffset = positionToOffset(source, startPos);
  const endOffset = positionToOffset(source, endPos);
  return source.slice(startOffset, endOffset);
}

export function QueryResults({ matches, format, source, fileName = 'input' }: QueryResultsProps) {
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
        <div key={i} className="match">
          {format === 'xml' && (
            <>
              {match.start && (
                <span className="match-location">
                  {match.start} - {match.end}
                </span>
              )}
              <pre><code>{match.xml.slice(0, 500)}{match.xml.length > 500 ? '...' : ''}</code></pre>
            </>
          )}
          {format === 'value' && (
            <pre><code>{match.value}</code></pre>
          )}
          {format === 'lines' && (
            <pre><code>{getSourceLines(source, match.start, match.end).join('\n')}</code></pre>
          )}
          {format === 'source' && (
            <pre><code>{getSourceSnippet(source, match.start, match.end)}</code></pre>
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
