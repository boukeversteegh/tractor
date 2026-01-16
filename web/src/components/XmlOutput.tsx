import { useCallback } from 'react';

interface XmlOutputProps {
  xml: string;
}

export function XmlOutput({ xml }: XmlOutputProps) {
  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(xml);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }, [xml]);

  if (!xml) {
    return <div className="xml-output empty">No XML output</div>;
  }

  return (
    <div className="xml-output">
      <div className="xml-toolbar">
        <button onClick={handleCopy}>Copy</button>
      </div>
      <pre><code>{xml}</code></pre>
    </div>
  );
}
