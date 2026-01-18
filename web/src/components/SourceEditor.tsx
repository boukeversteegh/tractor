import { useRef, useCallback, useMemo, useImperativeHandle, forwardRef } from 'react';
import { Match } from '../xpath';
import { parsePositionString, positionToOffset } from '../xmlTree';
import { highlightFullSourceSync, ansiToHtml } from '../tractor';

export interface SourceEditorHandle {
  focusAtOffset: (offset: number) => void;
}

interface HighlightRange {
  start: number;
  end: number;
  matchIndex: number;
}

interface SourceEditorProps {
  source: string;
  matches: Match[];
  hoveredMatchIndex?: number | null;
  xmlForHighlighting?: string;
  onChange: (source: string) => void;
  onClick: (e: React.MouseEvent<HTMLTextAreaElement>) => void;
}

export const SourceEditor = forwardRef<SourceEditorHandle, SourceEditorProps>(
  function SourceEditor({ source, matches, hoveredMatchIndex, xmlForHighlighting, onChange, onClick }, ref) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const backdropRef = useRef<HTMLDivElement>(null);

  // Expose focusAtOffset method via ref
  useImperativeHandle(ref, () => ({
    focusAtOffset: (offset: number) => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(offset, offset);
        // Scroll into view
        textareaRef.current.blur();
        textareaRef.current.focus();
      }
    },
  }), []);

  // Convert matches to highlight ranges (character offsets) with match indices
  const highlightRanges = useMemo((): HighlightRange[] => {
    const ranges: HighlightRange[] = [];

    matches.forEach((match, index) => {
      if (!match.start || !match.end) return;

      const startPos = parsePositionString(match.start);
      const endPos = parsePositionString(match.end);
      if (!startPos || !endPos) return;

      const startOffset = positionToOffset(source, startPos);
      const endOffset = positionToOffset(source, endPos);

      ranges.push({ start: startOffset, end: endOffset, matchIndex: index });
    });

    // Sort by start position (don't merge - we need to track individual matches)
    ranges.sort((a, b) => a.start - b.start);

    return ranges;
  }, [source, matches]);

  // Generate syntax-highlighted HTML from source + XML
  const syntaxHighlightedHtml = useMemo(() => {
    if (!xmlForHighlighting || !source) {
      return null;
    }
    const highlighted = highlightFullSourceSync(source, xmlForHighlighting);
    // Only use if we actually got highlighting (contains ANSI codes)
    if (highlighted && highlighted !== source && highlighted.includes('\x1b[')) {
      return ansiToHtml(highlighted);
    }
    return null;
  }, [source, xmlForHighlighting]);

  // Generate highlighted text with match markers (and optionally syntax highlighting)
  const highlightedContent = useMemo(() => {
    // If no match highlights needed, use syntax highlighting directly
    if (highlightRanges.length === 0) {
      return syntaxHighlightedHtml ?? escapeHtml(source);
    }

    // With match highlights, we need to combine them with syntax highlighting
    // For now, prioritize match highlights over syntax highlighting when both exist
    // TODO: Could combine them with a more sophisticated approach
    const parts: string[] = [];
    let lastEnd = 0;

    for (const range of highlightRanges) {
      // Add text before highlight (with syntax highlighting if available)
      if (range.start > lastEnd) {
        const segment = source.slice(lastEnd, range.start);
        parts.push(escapeHtml(segment));
      }

      // Add highlighted text with hover class if this is the hovered match
      const highlightedText = source.slice(range.start, range.end);
      const isHovered = hoveredMatchIndex === range.matchIndex;
      const className = isHovered ? 'highlight highlight-hovered' : 'highlight';
      parts.push(`<mark class="${className}">${escapeHtml(highlightedText)}</mark>`);

      lastEnd = range.end;
    }

    // Add remaining text
    if (lastEnd < source.length) {
      parts.push(escapeHtml(source.slice(lastEnd)));
    }

    return parts.join('');
  }, [source, syntaxHighlightedHtml, highlightRanges, hoveredMatchIndex]);

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
});

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
