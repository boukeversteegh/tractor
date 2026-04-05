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
        Modify values in your files — config, data, or source code. Tractor can update existing values, create missing structure, and even patch generated code. All in-place, across any language.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor set [FILES] -x <XPATH> --value <VALUE> [OPTIONS]
tractor set [FILES] <PATH_EXPRESSION> [--value <VALUE>] [OPTIONS]`} language="bash" />

      <h2>Update Config Values</h2>
      <p>
        The most common use: change a value in a config file. Tractor modifies the file in-place and preserves formatting:
      </p>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command={`tractor set config.json -x "//database/host" --value "localhost"`}
        output={`config.json:3: updated\nSet 1 match in 1 file`}
      />
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
        Use <code>--stdout</code> to see the result without touching the file:
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

      <h2>Update Multiple Files at Once</h2>
      <p>
        Distribute a value across many files. Useful for changing a database host, API URL, or version number everywhere at once:
      </p>
      <Example
        command={`tractor set env.yaml env-staging.yaml env-prod.yaml \\
    -x "//database/host" --value "new-db.example.com"`}
        output={`env.yaml:2: updated
env-staging.yaml:2: updated
env-prod.yaml:2: updated
Set 3 matches in 3 files`}
      />
      <p>Glob patterns work too:</p>
      <CodeBlock language="bash" code={`tractor set "config/**/*.yaml" -x "//database/host" --value "new-db.example.com"`} />

      <h2>Patch Source Code</h2>
      <p>
        Tractor isn't limited to config files. You can modify values in any source file — patch constants, update versions, change strings in generated code:
      </p>
      <Example
        file={{ name: 'version.js', language: 'js', content: `const VERSION = "0.9.0";
const APP_NAME = "myapp";` }}
        command={`tractor set version.js -x "//variable[name='VERSION']//string_fragment" \\
    --value "1.0.0" --stdout`}
        outputLanguage="js"
        output={`const VERSION = "1.0.0";
const APP_NAME = "myapp";`}
      />

      <h3>Fix generated code</h3>
      <p>
        When a code generator doesn't support a setting you need, patch the output:
      </p>
      <Example
        file={{ name: 'generated.js', language: 'js', content: `// Auto-generated - do not edit
const API_URL = "http://localhost:3000";
const TIMEOUT = "5000";
export { API_URL, TIMEOUT };` }}
        command={`tractor set generated.js \\
    -x "//variable[name='API_URL']//string_fragment" \\
    --value "https://api.production.com" --stdout`}
        outputLanguage="js"
        output={`// Auto-generated - do not edit
const API_URL = "https://api.production.com";
const TIMEOUT = "5000";
export { API_URL, TIMEOUT };`}
      />

      <h2>Create Missing Structure</h2>
      <p>
        When you set a path that doesn't exist yet, tractor creates the structure for you. Pass a path expression with values embedded — tractor builds the data structure that would match it:
      </p>
      <Example
        file={{ name: 'empty.json', language: 'json', content: '{}' }}
        command={`tractor set empty.json "database[host='localhost'][port='5432']"`}
        output={`  inserted database/host in empty.json
  inserted port in empty.json
Set 2 values in 1 file`}
      />
      <CodeBlock
        language="json"
        title="empty.json (after)"
        code={`{
  "database": {
    "host": "localhost",
    "port": "5432"
  }
}`}
      />
      <p>
        The path expression <code>database[host='localhost'][port='5432']</code> describes the desired structure — a <code>database</code> object with <code>host</code> and <code>port</code> children. Tractor creates whatever is missing.
      </p>

      <h3>Add to existing files</h3>
      <p>
        This also works on files that already have some structure — tractor adds only what's missing:
      </p>
      <Example
        file={{ name: 'app.json', language: 'json', content: `{
  "app": {
    "name": "myapp"
  }
}` }}
        command={`tractor set app.json "app[version='1.0'][debug='false']"`}
        output={`  inserted version in app.json
  inserted debug in app.json
Set 2 values in 1 file`}
      />
      <CodeBlock
        language="json"
        title="app.json (after)"
        code={`{
  "app": {
    "name": "myapp",
    "version": "1.0",
    "debug": "false"
  }
}`}
      />

      <h2>Works with JSON, YAML, TOML, and Code</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Example</th></tr>
        </thead>
        <tbody>
          <tr><td>JSON</td><td><code>tractor set config.json -x "//host" --value "localhost"</code></td></tr>
          <tr><td>YAML</td><td><code>tractor set config.yaml -x "//host" --value "localhost"</code></td></tr>
          <tr><td>TOML</td><td><code>tractor set config.toml -x "//host" --value "localhost"</code></td></tr>
          <tr><td>JavaScript</td><td><code>tractor set version.js -x "//variable[name='X']//string_fragment" --value "new"</code></td></tr>
        </tbody>
      </table>

      <h2>Use Cases</h2>
      <ul>
        <li><strong>Distribute config values</strong> — set the same database host, API key, or feature flag across multiple config files at once</li>
        <li><strong>Patch generated code</strong> — fix values in auto-generated files when the generator doesn't support an option</li>
        <li><strong>Update versions</strong> — change version strings in source files, package manifests, or config</li>
        <li><strong>Scaffold config</strong> — create missing structure in JSON/YAML files using path expressions</li>
        <li><strong>CI/CD variable injection</strong> — replace placeholder values with environment-specific settings before deployment</li>
      </ul>

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td>XPath expression to match nodes</td></tr>
          <tr><td><code>--value</code></td><td>New value to set (optional with path expressions)</td></tr>
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
