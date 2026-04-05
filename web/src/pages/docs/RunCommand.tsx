import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock, Example } from '../../components/CodeBlock';

const EXAMPLE_JS = `// TODO: fix this later
class UserRepository {
  getAllUsers() {
    return db.users;
  }

  saveUser(user) {
    db.users.push(user);
  }
}`;

export function RunCommand() {
  return (
    <DocLayout>
      <h1>run</h1>
      <p className="doc-lead">
        Execute a tractor config file with multiple rules and operations. This is the recommended way to use tractor in a project.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor run <CONFIG> [OPTIONS]`} language="bash" />

      <h2>Config File</h2>
      <p>
        A tractor config file defines rules, file patterns, and operations in YAML or TOML. Place it in your project root as <code>.tractor.yml</code>.
      </p>

      <h3>Minimal Example</h3>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: warning`}
      />
      <CodeBlock language="bash" code={`tractor run .tractor.yml`} />
      <OutputBlock output={`src/app.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~


1 warning in 1 file`} />

      <h3>Multiple Rules</h3>
      <Example
        file={{ name: 'example.js', language: 'js', content: EXAMPLE_JS }}
        command="tractor run .tractor.yml"
        output={`app.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~

example.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~

example.js:3:3: error: getAll methods in repositories should use orderBy
3 |   getAllUsers() {
      ^~~~~~~~~~~


1 error in 2 files`}
      />
      <p>With this config:</p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: warning

    - id: repository-needs-orderby
      xpath: >-
        //class[contains(name,'Repository')]
        //method[contains(name,'getAll')]
        [not(contains(.,'orderBy'))]/name
      reason: "getAll methods in repositories should use orderBy"
      severity: error`}
      />

      <h2>Rule Properties</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Property</th><th>Required</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>id</code></td><td>Yes</td><td>Unique identifier for the rule</td></tr>
          <tr><td><code>xpath</code></td><td>Yes</td><td>XPath expression — each match is a violation</td></tr>
          <tr><td><code>reason</code></td><td>Yes</td><td>Explanation shown for each violation</td></tr>
          <tr><td><code>severity</code></td><td>No</td><td><code>error</code> (default) or <code>warning</code></td></tr>
          <tr><td><code>message</code></td><td>No</td><td>Custom message template (<code>{'{value}'}</code>, <code>{'{line}'}</code>, etc.)</td></tr>
          <tr><td><code>include</code></td><td>No</td><td>File patterns for this rule only</td></tr>
          <tr><td><code>exclude</code></td><td>No</td><td>File patterns to exclude for this rule</td></tr>
          <tr><td><code>expect</code></td><td>No</td><td>Test examples (see below)</td></tr>
        </tbody>
      </table>

      <h2>Rule Testing with expect</h2>
      <p>
        Add <code>expect</code> entries to validate your rules directly in the config:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: error
      expect:
        - valid: "class Clean { }"
        - invalid: "// TODO: fix this"`}
      />
      <p>
        When you run <code>tractor run</code>, the <code>expect</code> entries are also validated. If a <code>valid</code> example matches the rule (or an <code>invalid</code> example doesn't), the run fails.
      </p>

      <h2>Multiple Operation Types</h2>
      <p>
        Use the <code>operations</code> list to mix check, test, query, and set operations:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`files:
  - "src/**/*.js"

operations:
  - check:
      rules:
        - id: no-todo
          xpath: "//comment[contains(.,'TODO')]"
          reason: "TODO comments should be resolved"
          severity: warning

  - test:
      assertions:
        - xpath: "//class"
          expect: some
          message: "At least one class expected"`}
      />

      <h2>Scope and File Resolution</h2>
      <p>
        File patterns can be set at multiple levels. Each level narrows the scope — it never widens it. The effective file set is the intersection of all levels that are defined.
      </p>

      <h3>Intersection chain</h3>
      <p>
        When you run <code>tractor run config.yaml frontend/**/*.js</code>, the file resolution works like this:
      </p>
      <CodeBlock
        language="text"
        code={`config root files  ∩  operation files  ∩  CLI files
     (broadest)         (per-operation)      (narrowest)`}
      />
      <ul>
        <li>If a level is <strong>not defined</strong>, it's skipped entirely (no intersection).</li>
        <li>If a level is defined but its <strong>patterns match nothing</strong>, the result is empty.</li>
      </ul>

      <h3>Root and operation files</h3>
      <CodeBlock
        language="yaml"
        code={`# Root-level: defines the broadest scope for all operations
files:
  - "src/**/*.js"
exclude:
  - "src/generated/**"

operations:
  - check:
      # This operation's files intersect with root files.
      # Effective: src/core/**/*.js that are also in src/**/*.js
      files:
        - "src/core/**/*.js"
      rules:
        - id: no-todo
          xpath: "//comment[contains(.,'TODO')]"
          reason: "No TODOs in core"

  - check:
      # No files specified — uses root files as the base.
      # Effective: src/**/*.js
      rules:
        - id: no-eval
          xpath: "//call[name='eval']"
          reason: "eval is not allowed"`}
      />
      <ul>
        <li><strong>files</strong>: Operation files intersect with root files — only files matching both patterns are processed.</li>
        <li><strong>No files on operation</strong>: Root files are used as the base.</li>
        <li><strong>exclude</strong>: Union of root and operation excludes (both narrow the scope).</li>
      </ul>

      <h3>CLI file arguments</h3>
      <p>
        Pass files or globs as positional arguments to narrow the config's scope to specific files:
      </p>
      <CodeBlock language="bash" code={`# Run config rules, but only on these files
tractor run .tractor.yml src/app.js src/utils.js

# Or with globs
tractor run .tractor.yml "src/core/**/*.js"`} />
      <p>
        CLI files are intersected with the resolved config scope. This is useful for checking only the files you changed, without modifying the config.
      </p>

      <h3>File limits</h3>
      <p>
        Tractor protects against accidentally globbing too many files:
      </p>
      <ul>
        <li><code>--max-files</code> (default: 10,000) — maximum files to process.</li>
        <li>Each glob pattern aborts if it expands past 10× the max-files limit.</li>
        <li>If all patterns match 0 files, tractor reports a fatal error instead of silently succeeding.</li>
      </ul>
      <CodeBlock language="bash" code={`# Increase the limit for large monorepos
tractor run .tractor.yml --max-files 50000`} />

      <h3>Debugging with --verbose</h3>
      <p>
        Use <code>--verbose</code> to see each file resolution step — which patterns are being expanded, the base directory, and how many files remain after each intersection:
      </p>
      <CodeBlock
        language="text"
        code={`$ tractor run .tractor.yml --verbose
  files: resolving relative to /home/user/project
  files: max 10000 files, expansion limit 100000
  files: expanding root scope "src/**/*.js" ...
  files: root scope has 342 file(s)
  files: expanding operation "src/core/**/*.js" ...
  files: operation has 48 file(s)
  files: 48 file(s) after root intersection (was 48)`}
      />

      <h2>Git-aware Filtering</h2>
      <p>
        Only check files or lines changed in a git diff:
      </p>
      <CodeBlock
        language="yaml"
        code={`# Only check files changed vs main branch
diff-files: "main..HEAD"

check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "No TODOs"`}
      />
      <p>
        You can also use <code>diff-lines</code> to restrict matches to changed hunks only, and override from the CLI with <code>--diff-files</code> or <code>--diff-lines</code>.
      </p>

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-f, --format</code></td><td>Output format: gcc (default), github, text, json, yaml, xml</td></tr>
          <tr><td><code>-v, --view</code></td><td>View fields to include</td></tr>
          <tr><td><code>-m, --message</code></td><td>Message template for matches</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
          <tr><td><code>--max-files</code></td><td>Maximum files to process (default: 10,000)</td></tr>
          <tr><td><code>--verbose</code></td><td>Show file resolution steps and diagnostics on stderr</td></tr>
          <tr><td><code>-c, --concurrency</code></td><td>Number of parallel workers</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/writing-queries">Writing Queries guide</Link> — learn to write tractor queries step by step.</p>
      </div>
    </DocLayout>
  );
}
