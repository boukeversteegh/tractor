import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

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
      <CodeBlock
        language="bash"
        code={`echo 'def greet(name):\n    return f"Hello, {name}"' | tractor -l python`}
      />
      <OutputBlock output={`<stdin>:1
<Files>
  <file>&lt;stdin&gt;</file>
  <unit>
    <function>
      def
      <name>greet</name>
      <parameters>
        (
        <parameter>
          <name>name</name>
        </parameter>
        )
      </parameters>
      <body>
        :
        <return>
          return
          ...
        </return>
      </body>
    </function>
  </unit>
</Files>`} />

      <h3>3. Query for patterns</h3>
      <p>
        Use <code>-x</code> to find specific elements:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'def greet(name):\n    return f"Hello, {name}"\n\ndef add(a, b):\n    return a + b' | tractor -l python -x "//function/name" -v value`}
      />
      <OutputBlock output={`greet\nadd`} />

      <h3>4. Enforce conventions</h3>
      <p>
        Use <code>tractor check</code> to fail your build when patterns are found:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO comments should be resolved"`}
      />
      <OutputBlock output={`src/app.cs:1:1: error: TODO comments should be resolved\n1 | // TODO: fix this later\n    ^~~~~~~~~~~~~~~~~~~~~~~\n\n\n1 error in 1 file`} />

      <h2>Core Concepts</h2>

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
        <Link to="/docs/commands/run" className="doc-card">
          <h3>run</h3>
          <p>Execute a config file with multiple rules and operations.</p>
        </Link>
      </div>

      <h2>Guides</h2>
      <div className="doc-cards">
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
      </div>
    </DocLayout>
  );
}
