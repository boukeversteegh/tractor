import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const CONFIG_JSON = `{
  "database": {
    "host": "old-server",
    "port": 5432
  }
}`;

const SETTINGS_YAML = `database:
  host: old-server
  port: 5432
debug: true`;

export function SetCommand() {
  return (
    <DocLayout>
      <h1>set</h1>
      <p className="doc-lead">
        Set matched node values in your files. Modifies files in-place by default — use <code>--stdout</code> to preview changes.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor set [FILES] -x <XPATH> --value <VALUE> [OPTIONS]`} language="bash" />

      <h2>Basic Set</h2>
      <p>
        Replace matched values in-place:
      </p>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command={`tractor set config.json -x "//database/host" --value "localhost"`}
        output={`config.json:3: updated
Set 1 match in 1 file`}
      />
      <p>After running, <code>config.json</code> is updated:</p>
      <CodeBlock
        language="json"
        title="config.json (after)"
        code={`{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}`}
      />

      <h2>Preview with --stdout</h2>
      <p>
        Use <code>--stdout</code> to see the result without modifying the file:
      </p>
      <Example
        file={{ name: 'settings.yaml', language: 'yaml', content: SETTINGS_YAML }}
        command={`tractor set settings.yaml -x "//database/host" --value "localhost" --stdout`}
        outputLanguage="yaml"
        output={`database:
  host: localhost
  port: 5432
debug: true`}
      />

      <h2>Works with JSON and YAML</h2>
      <p>
        Tractor auto-detects the file format and preserves structure when modifying values.
      </p>

      <h3>JSON</h3>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command={`tractor set config.json -x "//database/host" --value "localhost" --stdout`}
        outputLanguage="json"
        output={`{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}`}
      />

      <h3>YAML</h3>
      <Example
        file={{ name: 'settings.yaml', language: 'yaml', content: SETTINGS_YAML }}
        command={`tractor set settings.yaml -x "//debug" --value "false" --stdout`}
        outputLanguage="yaml"
        output={`database:
  host: old-server
  port: 5432
debug: false`}
      />

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td>XPath expression to match nodes</td></tr>
          <tr><td><code>--value</code></td><td>New value to set</td></tr>
          <tr><td><code>--stdout</code></td><td>Output to stdout instead of modifying in-place</td></tr>
          <tr><td><code>-v, --view</code></td><td>View: status (default), output, file, line, value, source, lines</td></tr>
          <tr><td><code>-f, --format</code></td><td>Output format: text (default), json, yaml, xml</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/commands/run">run command</Link> — execute a config file with multiple operations.</p>
      </div>
    </DocLayout>
  );
}
