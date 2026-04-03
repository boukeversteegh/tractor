import { useState } from 'react';
import { highlightCode } from '../highlight';

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

  const highlighted = highlightCode(code, language);

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
      <pre><code dangerouslySetInnerHTML={{ __html: highlighted }} /></pre>
    </div>
  );
}

interface OutputBlockProps {
  output: string;
  title?: string;
  language?: string;
}

export function OutputBlock({ output, title, language }: OutputBlockProps) {
  const highlighted = highlightCode(output, language);

  return (
    <div className="codeblock codeblock-output">
      <div className="codeblock-header">
        <span className="codeblock-title">{title || 'Output'}</span>
      </div>
      <pre><code dangerouslySetInnerHTML={{ __html: highlighted }} /></pre>
    </div>
  );
}

interface ExampleProps {
  /** The source file shown to the user */
  file?: { name: string; language: string; content: string };
  /** The tractor command to run */
  command: string;
  /** The verbatim output */
  output: string;
  /** Language hint for output highlighting (e.g. "xml") */
  outputLanguage?: string;
}

export function Example({ file, command, output, outputLanguage }: ExampleProps) {
  return (
    <div className="example-group">
      {file && (
        <CodeBlock code={file.content} language={file.language} title={file.name} />
      )}
      <CodeBlock code={command} language="bash" />
      <OutputBlock output={output} language={outputLanguage} />
    </div>
  );
}
