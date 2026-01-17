import { useRef, useCallback, useMemo } from 'react';

interface QueryInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  errorStart?: number;
  errorEnd?: number;
}

/**
 * XPath query input with error highlighting
 * Uses same pattern as SourceEditor - backdrop div with transparent input
 */
export function QueryInput({
  value,
  onChange,
  placeholder,
  className = '',
  errorStart,
  errorEnd,
}: QueryInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const backdropRef = useRef<HTMLDivElement>(null);

  // Sync scroll position between input and backdrop
  const handleScroll = useCallback(() => {
    if (inputRef.current && backdropRef.current) {
      backdropRef.current.scrollLeft = inputRef.current.scrollLeft;
    }
  }, []);

  // Generate highlighted content
  const highlightedContent = useMemo(() => {
    // Only show error highlight if we have valid positions (not undefined/null)
    const hasError = errorStart != null && errorEnd != null && value;
    if (!hasError) {
      return escapeHtml(value || '') || `<span class="placeholder">${escapeHtml(placeholder || '')}</span>`;
    }

    const parts: string[] = [];

    // Text before error
    if (errorStart > 0) {
      parts.push(escapeHtml(value.slice(0, errorStart)));
    }

    // Error highlight - show at least one character
    const errorText = value.slice(errorStart, Math.max(errorEnd, errorStart + 1)) || ' ';
    parts.push(`<mark class="error-highlight">${escapeHtml(errorText)}</mark>`);

    // Text after error
    if (errorEnd < value.length) {
      parts.push(escapeHtml(value.slice(errorEnd)));
    }

    return parts.join('');
  }, [value, errorStart, errorEnd, placeholder]);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    onChange(e.target.value);
  }, [onChange]);

  return (
    <div className={`query-input-wrapper ${className}`}>
      <div
        ref={backdropRef}
        className="query-input-backdrop"
        dangerouslySetInnerHTML={{ __html: highlightedContent }}
      />
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={handleChange}
        onScroll={handleScroll}
        placeholder={placeholder}
        className="query-input-transparent"
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
