import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

const APP_JS = `// TODO: fix this later
class App {
  run() { }
}`;

export function DocsOverview() {
  return (
    <DocLayout>
      <h1>Tractor Documentation</h1>
      <p className="doc-lead">
        Tractor lets development teams write rules that keep their codebase consistent — across any language, enforced automatically.
      </p>

      <h2>What is Tractor?</h2>
      <p>
        Tractor parses your source code into a tree, then lets you query it using standard expressions. You see exactly what you're querying — no hidden structure, no guessing. One tool, one syntax, 20+ languages.
      </p>

      <h2>Quick Start</h2>

      <h3>1. Install</h3>
      <p>
        Download the latest binary from the <a href="https://github.com/boukeversteegh/tractor/releases/latest" target="_blank" rel="noopener noreferrer">releases page</a>, or see the <Link to="/">homepage</Link> for platform-specific instructions.
      </p>

      <h3>2. Explore your code</h3>
      <p>
        Run <code>tractor</code> on a file to see its structure:
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
  │           └─ ... (5 children)
  └─ function/
      ├─ "function"
      ├─ name = "add"
      └─ ... (3 children)`}
      />

      <h3>3. Query for patterns</h3>
      <p>
        Use <code>-x</code> to find specific elements:
      </p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command={`tractor greeter.js -x "//function/name" -v value`}
        output={`greet\nadd`}
      />

      <h3>4. Enforce conventions</h3>
      <p>
        Use <code>tractor check</code> to fail your build when patterns are found:
      </p>
      <Example
        file={{ name: 'app.js', language: 'js', content: APP_JS }}
        command={`tractor check app.js -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO comments should be resolved"`}
        output={`app.js:1:1: error: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~


1 error in 1 file`}
      />

      <h2>How It Works</h2>
      <p>
        Tractor was designed to make querying code as simple as possible. Your code is parsed into a tree where:
      </p>
      <ul>
        <li><strong>Everything is a node</strong> — you match by element name, not attributes. Modifiers like <code>public</code> or <code>static</code> are empty marker elements, so you can filter with <code>[public]</code> or <code>[not(static)]</code>.</li>
        <li><strong>Text content is the source code</strong> — when you compare a node to a string, tractor matches against the flattened source text. This means you can write <code>{'//method[contains(.,"exit(1)")]'}</code> and it matches even though the source code spans multiple nested elements.</li>
        <li><strong>No attributes needed</strong> — the tree is structured so that nearly everything you'd want to query is a named element or text content, not a hidden attribute.</li>
      </ul>
      <p>
        Run <code>tractor file.js</code> to see the tree for yourself — what you see is exactly what you query.
      </p>

      <h2>Commands</h2>

      <div className="doc-cards">
        <Link to="/docs/commands/query" className="doc-card">
          <h3>query</h3>
          <p>Explore code structure and extract patterns from your source files.</p>
        </Link>
        <Link to="/docs/commands/check" className="doc-card">
          <h3>check</h3>
          <p>Run rules and report violations. Perfect for linting and CI.</p>
        </Link>
        <Link to="/docs/commands/test" className="doc-card">
          <h3>test</h3>
          <p>Assert match counts against expectations.</p>
        </Link>
        <Link to="/docs/commands/set" className="doc-card">
          <h3>set</h3>
          <p>Modify matched values in JSON, YAML, and other files.</p>
        </Link>
        <Link to="/docs/commands/run" className="doc-card">
          <h3>run</h3>
          <p>Execute a config file with multiple rules and operations.</p>
        </Link>
      </div>

      <h2>Supported Languages</h2>
      <p>
        Tractor supports 27 languages. Code languages are parsed into a syntax tree. Data formats (JSON, YAML, TOML, INI) are parsed into a data tree where keys become elements.
      </p>
      <p>
        <strong>Code:</strong> JavaScript, TypeScript, TSX, C#, Rust, Python, Go, Java, Ruby, C++, C, HTML, CSS, Bash, PHP, Scala, Lua, Haskell, OCaml, R, Julia, Markdown, T-SQL
      </p>
      <p>
        <strong>Data:</strong> <Link to="/docs/languages/data">JSON, YAML, TOML, INI</Link>
      </p>

      <h2>Guides</h2>
      <div className="doc-cards">
        <Link to="/docs/guides/query-syntax" className="doc-card">
          <h3>Query Syntax</h3>
          <p>Reference for path expressions, predicates, functions, and text matching.</p>
        </Link>
        <Link to="/docs/guides/writing-queries" className="doc-card">
          <h3>Writing Queries</h3>
          <p>Learn to write tractor queries step by step, from simple to advanced.</p>
        </Link>
        <Link to="/docs/guides/schema" className="doc-card">
          <h3>Exploring with Schema</h3>
          <p>Use the schema view to discover what elements are available to query.</p>
        </Link>
        <Link to="/docs/guides/lint-rules" className="doc-card">
          <h3>Writing Lint Rules</h3>
          <p>Create custom lint rules and bundle them into a rules file.</p>
        </Link>
        <Link to="/docs/guides/ci-cd" className="doc-card">
          <h3>CI/CD Integration</h3>
          <p>Set up tractor in GitHub Actions, GitLab CI, and other pipelines.</p>
        </Link>
        <Link to="/docs/guides/use-cases" className="doc-card">
          <h3>Use Cases</h3>
          <p>See how teams use tractor for conventions, security, architecture, AI guard railing, and config management.</p>
        </Link>
      </div>
    </DocLayout>
  );
}
