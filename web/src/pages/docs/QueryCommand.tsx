import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

const CONFIG_JSON = `{"database": {"host": "localhost", "port": 5432}}`;

export function QueryCommand() {
  return (
    <DocLayout>
      <h1>query</h1>
      <p className="doc-lead">
        Explore and extract data from your code's syntax tree. This is the default command — just run <code>tractor</code> with a file.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor [FILES] [OPTIONS]
tractor query [FILES] [OPTIONS]`} language="bash" />

      <h2>See the tree</h2>
      <p>
        Run tractor on a file to see its parsed structure. This is the first step — you see exactly what you're querying against.
      </p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command="tractor greeter.js"
        output={`greeter.js:1
program/
  ├─ function/
  │   ├─ "function"
  │   ├─ name = "greet"
  │   ├─ parameters/
  │   │   └─ params/
  │   │       ├─ "("
  │   │       ├─ type = "name"
  │   │       └─ ")"
  │   └─ body/
  │       └─ block/
  │           ├─ "{"
  │           ├─ return/
  │           │   ├─ "return"
  │           │   ├─ binary/
  │           │   │   ├─ op/plus = "+"
  │           │   │   ├─ left/string = "\\"Hello, \\""
  │           │   │   └─ right/type = "name"
  │           │   └─ ";"
  │           └─ "}"
  └─ function/
      ├─ "function"
      ├─ name = "add"
      ├─ parameters/
      │   └─ params/
      │       ├─ "("
      │       ├─ type = "a"
      │       ├─ ","
      │       ├─ type = "b"
      │       └─ ")"
      └─ body/
          └─ block/
              ├─ "{"
              ├─ return/
              │   ├─ "return"
              │   ├─ binary/
              │   │   ├─ op/plus = "+"
              │   │   ├─ left/type = "a"
              │   │   └─ right/type = "b"
              │   └─ ";"
              └─ "}"`}
      />

      <h2>Extract with -x</h2>
      <p>
        Use <code>-x</code> to query the tree and extract matching elements:
      </p>
      <Example
        command={`tractor greeter.js -x "//function/name" -v value`}
        output={`greet\nadd`}
      />

      <h2>Views</h2>
      <p>
        Use <code>-v</code> to control what you see. The default is <code>tree</code>, which shows the query-oriented tree view. Other views are more useful for specific tasks.
      </p>

      <h3>value</h3>
      <p>Extract the text content of matched nodes:</p>
      <Example
        command={`tractor greeter.js -x "//function/name" -v value`}
        output={`greet\nadd`}
      />

      <h3>source</h3>
      <p>Get the exact source code of matched nodes:</p>
      <Example
        command={`tractor greeter.js -x "//function" -v source`}
        outputLanguage="js"
        output={`function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`}
      />

      <h3>lines</h3>
      <p>Show the full source lines containing each match:</p>
      <Example
        command={`tractor greeter.js -x "//function/name" -v lines`}
        output={`1 | function greet(name) {
             ^~~~~


5 | function add(a, b) {
             ^~~`}
      />

      <h3>count</h3>
      <p>Count matches:</p>
      <Example
        command={`tractor greeter.js -x "//function" -v count`}
        output="2"
      />

      <h3>schema</h3>
      <p>See the structural overview of element types. Learn more in the <Link to="/docs/guides/schema">Schema guide</Link>.</p>
      <Example
        command={`tractor greeter.js -x "//function" -v schema`}
        output={`function (2)  function
├─ body (2)
│  └─ block (2)  {…}
│     └─ return (2)  return, ;
│        └─ binary (2)  +
│           └─ … (8 children)
├─ parameters (2)
│  └─ params (2)  (, ), ,
│     └─ type (3)  name, a, b
└─ name (2)  greet, add

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h2>Multi-language</h2>
      <p>Tractor works with 20+ languages. The same query syntax applies everywhere:</p>

      <h3>Rust</h3>
      <Example
        file={{ name: 'main.rs', language: 'rust', content: `fn main() {
    println!("Hello");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}` }}
        command={`tractor main.rs -x "//function/name" -v value`}
        output={`main\nadd`}
      />

      <h3>JSON</h3>
      <Example
        file={{ name: 'config.json', language: 'json', content: CONFIG_JSON }}
        command={`tractor config.json -x "//database/host" -v value`}
        output="localhost"
      />

      <h2>Output Formats</h2>
      <p>Use <code>-f</code> to change the output format:</p>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>text</code></td><td>Human-readable (default)</td></tr>
          <tr><td><code>json</code></td><td>JSON report</td></tr>
          <tr><td><code>yaml</code></td><td>YAML report</td></tr>
          <tr><td><code>xml</code></td><td>XML report</td></tr>
          <tr><td><code>gcc</code></td><td><code>file:line:col</code> format for editors</td></tr>
          <tr><td><code>github</code></td><td>GitHub Actions annotations</td></tr>
        </tbody>
      </table>

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

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td><Link to="/docs/guides/query-syntax">Query</Link> to match</td></tr>
          <tr><td><code>-v, --view</code></td><td>View mode: tree, value, source, lines, count, schema</td></tr>
          <tr><td><code>-f, --format</code></td><td>Output format: text, json, yaml, xml, gcc, github, claude-code</td></tr>
          <tr><td><code>-l, --lang</code></td><td>Language override (auto-detected from file extension)</td></tr>
          <tr><td><code>-s, --string</code></td><td>Inline source code (alternative to file)</td></tr>
          <tr><td><code>-n, --limit</code></td><td>Limit output to first N matches</td></tr>
          <tr><td><code>-d, --depth</code></td><td>Limit tree output depth</td></tr>
          <tr><td><code>-t, --tree</code></td><td>Tree mode: raw, structure, data</td></tr>
          <tr><td><code>-W</code></td><td>Ignore whitespace in string matching</td></tr>
          <tr><td><code>-g, --group</code></td><td>Group output by file</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
        </tbody>
      </table>
    </DocLayout>
  );
}
