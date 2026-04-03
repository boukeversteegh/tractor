import { useState } from 'react';

interface CodeBlockProps {
  code: string;
  language?: string;
  title?: string;
}

export function CodeBlock({ code, language, title }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="codeblock">
      {(title || language) && (
        <div className="codeblock-header">
          {title && <span className="codeblock-title">{title}</span>}
          {language && <span className="codeblock-lang">{language}</span>}
          <button className="codeblock-copy" onClick={handleCopy}>
            {copied ? 'Copied' : 'Copy'}
          </button>
        </div>
      )}
      {!title && !language && (
        <button className="codeblock-copy codeblock-copy-floating" onClick={handleCopy}>
          {copied ? 'Copied' : 'Copy'}
        </button>
      )}
      <pre><code>{code}</code></pre>
    </div>
  );
}

interface OutputBlockProps {
  output: string;
  title?: string;
}

export function OutputBlock({ output, title }: OutputBlockProps) {
  return (
    <div className="codeblock codeblock-output">
      <div className="codeblock-header">
        <span className="codeblock-title">{title || 'Output'}</span>
      </div>
      <pre><code>{output}</code></pre>
    </div>
  );
}
