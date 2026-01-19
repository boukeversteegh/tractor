import { useRef, useCallback, useMemo, useImperativeHandle, forwardRef } from 'react';
import { Match } from '../xpath';
import { XmlNode, parsePositionString, positionToOffset } from '../xmlTree';
import { highlightFullSourceSync, ansiToHtml } from '../tractor';

export interface SourceEditorHandle {
  focusAtOffset: (offset: number) => void;
}

interface HighlightRange {
  start: number;
  end: number;
  matchIndex: number;
  isHover?: boolean;  // true for tree node hover highlight
}

interface SourceEditorProps {
  source: string;
  matches: Match[];
  hoveredMatchIndex?: number | null;
  hoveredNode?: XmlNode | null;
  xmlForHighlighting?: string;
  language?: string;
  onChange: (source: string) => void;
  onClick: (e: React.MouseEvent<HTMLTextAreaElement>) => void;
}

export const SourceEditor = forwardRef<SourceEditorHandle, SourceEditorProps>(
  function SourceEditor({ source, matches, hoveredMatchIndex, hoveredNode, xmlForHighlighting, language, onChange, onClick }, ref) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const syntaxLayerRef = useRef<HTMLDivElement>(null);
  const matchLayerRef = useRef<HTMLDivElement>(null);

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

    // Sort by start position
    ranges.sort((a, b) => a.start - b.start);

    return ranges;
  }, [source, matches]);

  // Layer 1: Syntax-highlighted text (foreground)
  const syntaxContent = useMemo(() => {
    if (!xmlForHighlighting || !source) {
      return escapeHtml(source);
    }
    const highlighted = highlightFullSourceSync(source, xmlForHighlighting, language || '');
    if (highlighted && highlighted !== source && highlighted.includes('\x1b[')) {
      return ansiToHtml(highlighted);
    }
    return escapeHtml(source);
  }, [source, xmlForHighlighting, language]);

  // Combine match ranges with hovered node range
  const allHighlightRanges = useMemo((): HighlightRange[] => {
    const ranges = [...highlightRanges];

    // Add hovered tree node range
    if (hoveredNode?.location) {
      const { start, end } = hoveredNode.location;
      const startOffset = positionToOffset(source, start);
      const endOffset = positionToOffset(source, end);
      ranges.push({ start: startOffset, end: endOffset, matchIndex: -1, isHover: true });
    }

    // Sort by start position
    ranges.sort((a, b) => a.start - b.start);
    return ranges;
  }, [highlightRanges, hoveredNode, source]);

  // Layer 2: Highlights (background) - same text but with <mark> tags
  const highlightsContent = useMemo(() => {
    if (allHighlightRanges.length === 0) {
      return null; // No highlights layer needed
    }

    const parts: string[] = [];
    let lastEnd = 0;

    for (const range of allHighlightRanges) {
      // Skip ranges completely contained within already-processed text
      if (range.end <= lastEnd) {
        continue;
      }

      // Adjust start if it overlaps with already-processed text
      const effectiveStart = Math.max(range.start, lastEnd);

      // Add gap text before this highlight
      if (effectiveStart > lastEnd) {
        parts.push(escapeHtml(source.slice(lastEnd, effectiveStart)));
      }

      const text = source.slice(effectiveStart, range.end);
      let className: string;
      if (range.isHover) {
        className = 'highlight highlight-node';
      } else {
        const isHovered = hoveredMatchIndex === range.matchIndex;
        className = isHovered ? 'highlight highlight-hovered' : 'highlight';
      }
      parts.push(`<mark class="${className}">${escapeHtml(text)}</mark>`);

      lastEnd = range.end;
    }

    if (lastEnd < source.length) {
      parts.push(escapeHtml(source.slice(lastEnd)));
    }

    return parts.join('');
  }, [source, allHighlightRanges, hoveredMatchIndex]);

  // Sync scroll between textarea and both layers
  const handleScroll = useCallback(() => {
    if (textareaRef.current) {
      const { scrollTop, scrollLeft } = textareaRef.current;
      if (syntaxLayerRef.current) {
        syntaxLayerRef.current.scrollTop = scrollTop;
        syntaxLayerRef.current.scrollLeft = scrollLeft;
      }
      if (matchLayerRef.current) {
        matchLayerRef.current.scrollTop = scrollTop;
        matchLayerRef.current.scrollLeft = scrollLeft;
      }
    }
  }, []);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
  }, [onChange]);

  return (
    <div className="source-editor">
      {/* Layer 1 (bottom): Match highlight backgrounds */}
      {highlightsContent && (
        <div
          ref={matchLayerRef}
          className="source-highlights"
          dangerouslySetInnerHTML={{ __html: highlightsContent }}
        />
      )}
      {/* Layer 2 (middle): Syntax-colored text */}
      <div
        ref={syntaxLayerRef}
        className="source-syntax"
        dangerouslySetInnerHTML={{ __html: syntaxContent }}
      />
      {/* Layer 3 (top): Editable textarea */}
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
