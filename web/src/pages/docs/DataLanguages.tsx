import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const CONFIG_JSON = `{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "debug": true
}`;

const SETTINGS_YAML = `database:
  host: localhost
  port: 5432
debug: true
log_level: verbose`;

const CONFIG_TOML = `[database]
host = "localhost"
port = 5432`;

export function DataLanguages() {
  return (
    <DocLayout>
      <h1>Data Languages</h1>
      <p className="doc-lead">
        JSON, YAML, TOML, and INI files are parsed into a data tree where keys become elements and values become text. You query them the same way as code — but the tree is shaped like the data, not like syntax.
      </p>

      <h2>How It Works</h2>
      <p>
        For data formats, tractor transforms the file into a tree that mirrors its structure directly. Object keys become element names, values become text content. There are no syntax nodes like "string" or "pair" — just your data.
      </p>

      <h3>JSON</h3>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command="tractor config.json"
        outputLanguage="xml"
        output={`config.json:1
<Files>
  <file>config.json</file>
  <database>
    <host>localhost</host>
    <port>5432</port>
  </database>
  <debug>true</debug>
</Files>`}
      />
      <p>
        The JSON structure maps directly: <code>database.host</code> becomes <code>&lt;database&gt;&lt;host&gt;</code>. Querying is intuitive:
      </p>
      <Example
        command={`tractor config.json -x "//database/host" -v value`}
        output="localhost"
      />

      <h3>YAML</h3>
      <Example
        file={{ name: 'settings.yaml', language: 'yaml', content: SETTINGS_YAML }}
        command="tractor settings.yaml"
        outputLanguage="xml"
        output={`settings.yaml:1
<Files>
  <file>settings.yaml</file>
  <document>
    <database>
      <host>localhost</host>
      <port>5432</port>
    </database>
    <debug>true</debug>
    <log_level>verbose</log_level>
  </document>
</Files>`}
      />
      <p>
        YAML files have a <code>&lt;document&gt;</code> wrapper. Queries work the same:
      </p>
      <Example
        command={`tractor settings.yaml -x "//database/port" -v value`}
        output="5432"
      />

      <h3>TOML</h3>
      <Example
        file={{ name: 'config.toml', language: 'toml', content: CONFIG_TOML }}
        command="tractor config.toml"
        outputLanguage="xml"
        output={`config.toml:1
<Files>
  <file>config.toml</file>
  <document>
    <database>
      <host>localhost</host>
      <port>5432</port>
    </database>
  </document>
</Files>`}
      />
      <Example
        command={`tractor config.toml -x "//database/host" -v value`}
        output="localhost"
      />

      <h3>INI</h3>
      <p>INI files work the same way — sections become parent elements, keys become children.</p>
      <CodeBlock language="bash" code={`echo '[database]\nhost = localhost\nport = 5432' | tractor -l ini -x "//database/host" -v value`} />

      <h2>Arrays</h2>
      <p>
        Array items become repeated elements with the same name. In JSON, array elements inside an object key repeat that key's element name:
      </p>
      <Example
        file={{ name: 'servers.json', language: 'json', content: `{
  "servers": [
    {"host": "web1", "port": 80},
    {"host": "web2", "port": 443}
  ]
}` }}
        command="tractor servers.json"
        outputLanguage="xml"
        output={`servers.json:1
<Files>
  <file>servers.json</file>
  <servers>
    <host>web1</host>
    <port>80</port>
  </servers>
  <servers>
    <host>web2</host>
    <port>443</port>
  </servers>
</Files>`}
      />
      <p>
        Each array item becomes a separate <code>&lt;servers&gt;</code> element. You can query across all of them or filter:
      </p>
      <Example
        command={`tractor servers.json -x "//servers/host" -v value`}
        output={`web1\nweb2`}
      />
      <Example
        command={`tractor servers.json -x "//servers[port='443']/host" -v value`}
        output="web2"
      />

      <p>YAML arrays work the same way:</p>
      <Example
        file={{ name: 'replicas.yaml', language: 'yaml', content: `database:
  replicas:
    - host: replica1
    - host: replica2` }}
        command={`tractor replicas.yaml -x "//replicas/host" -v value`}
        output={`replica1\nreplica2`}
      />

      <h2>Enforcing Config Rules</h2>
      <p>
        Use <code>tractor check</code> on data files to enforce configuration policies:
      </p>
      <Example
        file={{ name: 'settings.yaml', language: 'yaml', content: SETTINGS_YAML }}
        command={`tractor check settings.yaml -x "//debug[.='true']" \\
    --reason "debug mode must be disabled"`}
        output={`settings.yaml:4:8: error: debug mode must be disabled
4 | debug: true
           ^~~~


1 error in 1 file`}
      />

      <h2>Modifying Config Values</h2>
      <p>
        Use <code>tractor set</code> to update values in-place:
      </p>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command={`tractor set config.json -x "//database/host" --value "production-db" --stdout`}
        outputLanguage="json"
        output={`{
  "database": {
    "host": "production-db",
    "port": 5432
  },
  "debug": true
}`}
      />

      <h2>Code vs Data</h2>
      <p>
        Tractor auto-detects whether to use a code tree or data tree based on the file extension. You can override this with <code>-t</code>:
      </p>
      <table className="doc-table">
        <thead>
          <tr><th>Mode</th><th>Used for</th><th>Tree shape</th></tr>
        </thead>
        <tbody>
          <tr><td><code>structure</code></td><td>Code languages (JS, Rust, C#, etc.)</td><td>Syntax elements: function, class, method, parameters, etc.</td></tr>
          <tr><td><code>data</code></td><td>JSON, YAML, TOML, INI</td><td>Data mirrors file structure: keys are elements, values are text</td></tr>
          <tr><td><code>raw</code></td><td>Any (manual override)</td><td>Raw parser output with no transforms</td></tr>
        </tbody>
      </table>
      <p>
        To see the raw parser output for a data file (before data transforms), use <code>-t raw</code>.
      </p>

      <h2>Supported Data Formats</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Extensions</th></tr>
        </thead>
        <tbody>
          <tr><td>JSON</td><td><code>.json</code></td></tr>
          <tr><td>YAML</td><td><code>.yaml</code>, <code>.yml</code></td></tr>
          <tr><td>TOML</td><td><code>.toml</code></td></tr>
          <tr><td>INI</td><td><code>.ini</code></td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/query-syntax">Query Syntax</Link> — learn the full query language.</p>
      </div>
    </DocLayout>
  );
}
