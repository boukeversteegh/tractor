import { useRef, useCallback, useMemo } from 'react';
import { Match } from '../xpath';
import { parsePositionString, positionToOffset } from '../xmlTree';

interface HighlightRange {
  start: number;
  end: number;
}

interface SourceEditorProps {
  source: string;
  matches: Match[];
  onChange: (source: string) => void;
  onClick: (e: React.MouseEvent<HTMLTextAreaElement>) => void;
}

export function SourceEditor({ source, matches, onChange, onClick }: SourceEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const backdropRef = useRef<HTMLDivElement>(null);

  // Convert matches to highlight ranges (character offsets)
  const highlightRanges = useMemo((): HighlightRange[] => {
    const ranges: HighlightRange[] = [];

    console.log('SourceEditor matches:', matches);

    for (const match of matches) {
      console.log('Match:', match.start, match.end, 'xml:', match.xml.substring(0, 100));
      if (!match.start || !match.end) continue;

      const startPos = parsePositionString(match.start);
      const endPos = parsePositionString(match.end);
      console.log('Parsed positions:', startPos, endPos);
      if (!startPos || !endPos) continue;

      const startOffset = positionToOffset(source, startPos);
      const endOffset = positionToOffset(source, endPos);
      console.log('Offsets:', startOffset, endOffset);

      ranges.push({ start: startOffset, end: endOffset });
    }

    console.log('Highlight ranges:', ranges);

    // Sort by start position and merge overlapping ranges
    ranges.sort((a, b) => a.start - b.start);

    const merged: HighlightRange[] = [];
    for (const range of ranges) {
      if (merged.length === 0) {
        merged.push(range);
      } else {
        const last = merged[merged.length - 1];
        if (range.start <= last.end) {
          // Overlapping - extend the last range
          last.end = Math.max(last.end, range.end);
        } else {
          merged.push(range);
        }
      }
    }

    return merged;
  }, [source, matches]);

  // Generate highlighted text with mark tags
  const highlightedContent = useMemo(() => {
    if (highlightRanges.length === 0) {
      return escapeHtml(source);
    }

    const parts: string[] = [];
    let lastEnd = 0;

    for (const range of highlightRanges) {
      // Add text before highlight
      if (range.start > lastEnd) {
        parts.push(escapeHtml(source.slice(lastEnd, range.start)));
      }

      // Add highlighted text
      const highlightedText = source.slice(range.start, range.end);
      parts.push(`<mark class="highlight">${escapeHtml(highlightedText)}</mark>`);

      lastEnd = range.end;
    }

    // Add remaining text
    if (lastEnd < source.length) {
      parts.push(escapeHtml(source.slice(lastEnd)));
    }

    return parts.join('');
  }, [source, highlightRanges]);

  // Sync scroll between textarea and backdrop
  const handleScroll = useCallback(() => {
    if (textareaRef.current && backdropRef.current) {
      backdropRef.current.scrollTop = textareaRef.current.scrollTop;
      backdropRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
  }, [onChange]);

  return (
    <div className="source-editor">
      <div
        ref={backdropRef}
        className="source-backdrop"
        dangerouslySetInnerHTML={{ __html: highlightedContent }}
      />
      <textarea
        ref={textareaRef}
        value={source}
        onChange={handleChange}
        onClick={onClick}
        onScroll={handleScroll}
        placeholder="Enter source code..."
        className="source-textarea"
        spellCheck={false}
      />
    </div>
  );
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
