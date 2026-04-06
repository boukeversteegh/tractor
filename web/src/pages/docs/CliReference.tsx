import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

export function CliReference() {
  return (
    <DocLayout>
      <h1>CLI Reference</h1>
      <p className="doc-lead">
        Every option available in <code>tractor</code>, with examples.
      </p>

      <h2>Input</h2>

      <h3>FILES</h3>
      <p>One or more files or glob patterns to process. Tractor auto-detects the language from the file extension.</p>
      <CodeBlock language="bash" code={`tractor greeter.js
tractor "src/**/*.js"
tractor file1.js file2.js`} />

      <h3>-l, --lang</h3>
      <p>Language for stdin input. Not needed when using files (language is auto-detected from extension).</p>
      <CodeBlock language="bash" code={`echo 'function f() {}' | tractor -l javascript -x "//function/name" -v value`} />
      <OutputBlock output="f" />

      <h3>-s, --string</h3>
      <p>Parse an inline source string instead of reading from a file or stdin. Requires <code>-l</code>.</p>
      <Example
        command={`tractor -s 'function hello() { }' -l javascript -x "//function/name" -v value`}
        output="hello"
      />

      <h2>Extract</h2>

      <h3>-x, --extract</h3>
      <p>XPath expression to select matching AST nodes. Without this, the full tree is shown.</p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command={`tractor greeter.js -x "//function/name" -v value`}
        output={`greet\nadd`}
      />

      <h3>-t, --tree</h3>
      <p>Tree mode — controls how source code is parsed into a tree.</p>
      <table className="doc-table">
        <thead>
          <tr><th>Mode</th><th>Description</th><th>Default for</th></tr>
        </thead>
        <tbody>
          <tr><td><code>structure</code></td><td>Semantic syntax tree with transforms</td><td>Code languages (JS, Rust, etc.)</td></tr>
          <tr><td><code>data</code></td><td>Data projection — keys become elements, values become text</td><td>JSON, YAML, TOML, INI</td></tr>
          <tr><td><code>raw</code></td><td>Raw parser output, no transforms (advanced)</td><td>—</td></tr>
        </tbody>
      </table>
      <p>
        You can override the default. For example, use <code>-t structure</code> on a JSON file to see its full syntax tree instead of the data projection:
      </p>
      <Example
        file={{ name: 'config.json', language: 'json', content: '{"host": "localhost", "port": 5432}' }}
        command={`tractor config.json -t structure`}
        outputLanguage="xml"
        output={`config.json:1
<Files>
  <file>config.json</file>
  <object>
    <property>
      <key>
        <string>host</string>
      </key>
      <value>
        <string>localhost</string>
      </value>
    </property>
    <property>
      <key>
        <string>port</string>
      </key>
      <value>
        <number>5432</number>
      </value>
    </property>
  </object>
</Files>`}
      />
      <p>Compare with the default <code>data</code> mode, where the same JSON becomes a clean data tree:</p>
      <Example
        command={`tractor config.json`}
        outputLanguage="xml"
        output={`config.json:1
<Files>
  <file>config.json</file>
  <host>localhost</host>
  <port>5432</port>
</Files>`}
      />

      <h3>-W, --ignore-whitespace</h3>
      <p>Ignore whitespace when comparing strings in XPath. Useful when source code has varying formatting.</p>
      <Example
        command={`echo 'function greet( name ) { return name; }' | tractor -l javascript -x "//params[.='(name)']" -v value -W`}
        output="(name)"
      />
      <p>Without <code>-W</code>, the match would fail because the source has spaces: <code>( name )</code>.</p>

      <h2>View</h2>

      <h3>-v, --view</h3>
      <p>Controls what output you see for each match.</p>
      <table className="doc-table">
        <thead>
          <tr><th>View</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>tree</code></td><td>Full parsed XML (default)</td></tr>
          <tr><td><code>value</code></td><td>Text content of matched nodes</td></tr>
          <tr><td><code>source</code></td><td>Exact matched source code</td></tr>
          <tr><td><code>lines</code></td><td>Full source lines containing each match</td></tr>
          <tr><td><code>count</code></td><td>Total number of matches</td></tr>
          <tr><td><code>schema</code></td><td>Structural overview of element types</td></tr>
          <tr><td><code>query</code></td><td>Echo the XPath query (useful for debugging shell escaping)</td></tr>
        </tbody>
      </table>
      <Example
        command={`tractor greeter.js -x "//function/name" -v lines`}
        output={`1 | function greet(name) {
             ^~~~~


5 | function add(a, b) {
             ^~~`}
      />

      <h3>-n, --limit</h3>
      <p>Limit output to the first N matches.</p>
      <Example
        command={`tractor greeter.js -x "//function" -v source -n 1`}
        outputLanguage="js"
        output={`function greet(name) {
  return "Hello, " + name;
}`}
      />

      <h3>-d, --depth</h3>
      <p>Limit XML output depth. Deeper elements are collapsed into comments. Default is 4 for schema view.</p>
      <Example
        command={`tractor greeter.js -d 3`}
        outputLanguage="xml"
        output={`greeter.js:1
<Files>
  <file>greeter.js</file>
  <program>
    <function>
      function
      <name>greet</name>
      <parameters>
        <!-- ... (2 children) -->
      </parameters>
      <body>
        <!-- ... (10 children) -->
      </body>
    </function>
    <function>
      function
      <name>add</name>
      <parameters>
        <!-- ... (3 children) -->
      </parameters>
      <body>
        <!-- ... (9 children) -->
      </body>
    </function>
  </program>
</Files>`}
      />

      <h3>--meta</h3>
      <p>Include metadata attributes (start/end positions, node kind, field name) in XML output.</p>
      <Example
        command={`tractor greeter.js -x "//function/name" --meta`}
        outputLanguage="xml"
        output={`greeter.js:1
<name start="1:10" end="1:15" field="name">greet</name>

greeter.js:5
<name start="5:10" end="5:13" field="name">add</name>`}
      />

      <h3>-g, --group</h3>
      <p>Group output by file. Most visible in JSON format.</p>
      <Example
        command={`tractor a.js b.js -x "//function/name" -g file -f json`}
        outputLanguage="json"
        output={`{
  "group": "file",
  "results": [
    {
      "file": "a.js",
      "results": [
        { "line": 1, "tree": "hello" }
      ]
    },
    {
      "file": "b.js",
      "results": [
        { "line": 1, "tree": "world" }
      ]
    }
  ]
}`}
      />

      <h3>-m, --message</h3>
      <p>Custom message template for each match. Available variables: <code>{'{value}'}</code>, <code>{'{line}'}</code>, <code>{'{col}'}</code>, <code>{'{file}'}</code>.</p>
      <Example
        command={`tractor greeter.js -x "//function/name" -m "Found: {value} at line {line}"`}
        output={`Found: greet at line 1\nFound: add at line 5`}
      />

      <h2>Format</h2>

      <h3>-f, --format</h3>
      <p>Output serialization format.</p>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Description</th><th>Default for</th></tr>
        </thead>
        <tbody>
          <tr><td><code>text</code></td><td>Human-readable plain text</td><td>query, test</td></tr>
          <tr><td><code>json</code></td><td>JSON report envelope</td><td>—</td></tr>
          <tr><td><code>yaml</code></td><td>YAML report envelope</td><td>—</td></tr>
          <tr><td><code>xml</code></td><td>XML report envelope</td><td>—</td></tr>
          <tr><td><code>gcc</code></td><td><code>file:line:col: severity: reason</code></td><td>check, run</td></tr>
          <tr><td><code>github</code></td><td>GitHub Actions annotations</td><td>—</td></tr>
          <tr><td><code>claude-code</code></td><td>Claude Code hook JSON (use with <code>--hook</code>)</td><td>—</td></tr>
        </tbody>
      </table>

      <h4>JSON</h4>
      <Example
        command={`tractor greeter.js -x "//function/name" -v value -f json`}
        outputLanguage="json"
        output={`{
  "results": [
    {
      "value": "greet"
    },
    {
      "value": "add"
    }
  ]
}`}
      />

      <h4>YAML</h4>
      <Example
        command={`tractor greeter.js -x "//function/name" -v value -f yaml`}
        outputLanguage="yaml"
        output={`results:
- value: greet
- value: add`}
      />

      <h4>XML</h4>
      <Example
        command={`tractor greeter.js -x "//function/name" -v value -f xml`}
        outputLanguage="xml"
        output={`<?xml version="1.0" encoding="UTF-8"?>
<report>
  <results>
    <match file="greeter.js" line="1" column="10" end_line="1" end_column="15">
      <value>greet</value>
    </match>
    <match file="greeter.js" line="5" column="10" end_line="5" end_column="13">
      <value>add</value>
    </match>
  </results>
</report>`}
      />

      <h4>GitHub</h4>
      <Example
        command={`tractor check app.js -x "//comment[contains(.,'TODO')]" --reason "TODO found" -f github`}
        output={`::error file=app.js,line=1,endLine=1,col=1,endColumn=24::TODO found`}
      />

      <h3>--no-pretty</h3>
      <p>Disable pretty printing. Shows compact XML without indentation.</p>
      <Example
        file={{ name: 'small.json', language: 'json', content: '{"host": "localhost"}' }}
        command={`tractor small.json --no-pretty`}
        output={`small.json:1\n<Files><file>small.json</file><host>localhost</host></Files>`}
      />

      <h3>--color</h3>
      <p>Control color output: <code>auto</code> (default), <code>always</code>, or <code>never</code>.</p>
      <CodeBlock language="bash" code={`tractor greeter.js --color never
tractor greeter.js --color always | less -R`} />

      <h3>--no-color</h3>
      <p>Shorthand for <code>--color never</code>. Also respects the <code>NO_COLOR</code> environment variable.</p>

      <h2>Filter</h2>

      <h3>--diff-files</h3>
      <p>Only process files changed in a git diff range. Useful in CI to only check modified files.</p>
      <CodeBlock language="bash" code={`# Only check files changed vs main
tractor check "src/**/*.js" --diff-files "main..HEAD" -x "//comment[contains(.,'TODO')]" --reason "TODO"`} />

      <h3>--diff-lines</h3>
      <p>Only report matches in changed hunks of a git diff. Even more targeted than <code>--diff-files</code>.</p>
      <CodeBlock language="bash" code={`# Only flag TODOs in newly written code
tractor check "src/**/*.js" --diff-lines "main..HEAD" -x "//comment[contains(.,'TODO')]" --reason "TODO"`} />

      <h2>Advanced</h2>

      <h3>-c, --concurrency</h3>
      <p>Number of parallel workers for processing files. Defaults to the number of CPU cores.</p>
      <CodeBlock language="bash" code={`tractor "src/**/*.js" -x "//function" -v count -c 4`} />

      <h3>--verbose</h3>
      <p>Show detailed output, including which files are being processed.</p>
      <CodeBlock language="bash" code={`tractor "src/**/*.js" -x "//function" --verbose`} />

      <h3>--debug</h3>
      <p>Show the full XML with match highlights and metadata attributes. Useful for debugging XPath queries.</p>
      <Example
        command={`echo 'function f() {}' | tractor -l javascript -x "//function" --debug`}
        outputLanguage="xml"
        output={`<stdin>:1
<function kind="function_declaration" start="1:1" end="1:16">
  function
  <name start="1:10" end="1:11" field="name">f</name>
  <parameters start="1:11" end="1:13" field="parameters">
    <params kind="formal_parameters" start="1:11" end="1:13" field="params">()</params>
  </parameters>
  <body start="1:14" end="1:16" field="body">
    <block kind="statement_block" start="1:14" end="1:16" field="block">{}</block>
  </body>
</function>`}
      />

      <h3>--parse-depth</h3>
      <p>[Experimental] Limit tree building depth. Skips parsing deeper nodes for speed on large files.</p>
      <CodeBlock language="bash" code={`tractor large-file.js -x "//class/name" -v value --parse-depth 5`} />

      <h3>-V, --version</h3>
      <p>Print version information. Add <code>--verbose</code> for detailed library versions.</p>
      <Example
        command="tractor -V"
        output={`tractor 0.1.0

Core libraries:
  tree-sitter  0.26.3
  xot          0.31.2
  xee-xpath    0.1.5 (git)`}
      />

      <h2>Check-specific Options</h2>

      <h3>--reason</h3>
      <p>Reason message shown for each violation.</p>
      <CodeBlock language="bash" code={`tractor check "src/**/*.js" -x "//comment[contains(.,'TODO')]" --reason "Resolve TODO before merging"`} />

      <h3>--severity</h3>
      <p>Severity level: <code>error</code> (default, fails build) or <code>warning</code> (passes build).</p>
      <CodeBlock language="bash" code={`tractor check "src/**/*.js" -x "//comment[contains(.,'TODO')]" --reason "TODO" --severity warning`} />

      <h3>--rules</h3>
      <p>Path to a YAML/TOML rules file for batch checking.</p>
      <CodeBlock language="bash" code={`tractor check "src/**/*.js" --rules rules.yaml`} />

      <h3>--expect-valid / --expect-invalid</h3>
      <p>Inline code examples to validate that your rule works correctly.</p>
      <CodeBlock language="bash" code={`tractor check "src/**/*.js" \\
    -x "//comment[contains(.,'TODO')]" --reason "No TODOs" \\
    --expect-valid 'class Clean { }' \\
    --expect-invalid '// TODO: fix' \\
    -l javascript`} />

      <h2>Test-specific Options</h2>

      <h3>-e, --expect</h3>
      <p>Expected match result: <code>none</code>, <code>some</code>, or an exact number.</p>
      <CodeBlock language="bash" code={`tractor test greeter.js -x "//function" --expect 2 -m "Expected 2 functions"
tractor test greeter.js -x "//class" --expect none -m "No classes"
tractor test greeter.js -x "//function" --expect some -m "Has functions"`} />

      <h2>Set-specific Options</h2>

      <h3>--value</h3>
      <p>New value to set on matched nodes.</p>
      <CodeBlock language="bash" code={`tractor set config.json -x "//database/host" --value "localhost"`} />

      <h3>--stdout</h3>
      <p>Write output to stdout instead of modifying the file in-place. Useful for previewing changes.</p>
      <CodeBlock language="bash" code={`tractor set config.json -x "//database/host" --value "localhost" --stdout`} />
    </DocLayout>
  );
}
