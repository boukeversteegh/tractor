import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const APP_JS = `// TODO: fix this later
class App {
  run() { }
}`;

export function CheckCommand() {
  return (
    <DocLayout>
      <h1>check</h1>
      <p className="doc-lead">
        Run rules and report violations. Exits with a non-zero code when errors are found — perfect for CI and pre-commit hooks.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor check [FILES] -x <XPATH> --reason <REASON> [OPTIONS]`} language="bash" />

      <h2>Basic Check</h2>
      <p>
        Flag code patterns that violate your conventions. Every match is a violation:
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
      <p>
        The exit code is <code>1</code> when errors are found, <code>0</code> when the check passes.
      </p>

      <h2>Severity Levels</h2>
      <p>
        Use <code>--severity</code> to set the level. Warnings don't fail the build:
      </p>
      <Example
        command={`tractor check app.js -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO comment found" --severity warning`}
        output={`app.js:1:1: warning: TODO comment found
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~


1 warning in 1 file`}
      />
      <p>
        With <code>--severity warning</code>, the exit code is <code>0</code> even when matches are found.
      </p>

      <h2>Output Formats</h2>
      <p>
        The default format for <code>check</code> is <code>gcc</code> (file:line:col), which works with most editors and CI systems. Use <code>-f</code> to change it.
      </p>

      <h3>GitHub Actions</h3>
      <p>
        Use <code>-f github</code> to produce annotations that show directly on pull requests:
      </p>
      <Example
        command={`tractor check app.js -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO comment found" -f github`}
        output={`::error file=app.js,line=1,endLine=1,col=1,endColumn=24::TODO comment found`}
      />

      <h2>Rules Files</h2>
      <p>
        Bundle multiple rules into a YAML file with <code>--rules</code>:
      </p>
      <p>
        <em>Note: the rules file format is being unified with the <code>tractor run</code> config file. In a future release, you'll be able to use the same <code>.tractor.yml</code> for both. The file name is free to choose — any <code>.yaml</code> or <code>.toml</code> file works.</em>
      </p>
      <CodeBlock
        language="yaml"
        title="rules.yaml"
        code={`rules:
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
      <CodeBlock
        language="bash"
        code={`tractor check "src/**/*.js" --rules rules.yaml`}
      />

      <h2>Testing Rules Inline</h2>
      <p>
        Validate that your rule works correctly with <code>--expect-valid</code> and <code>--expect-invalid</code>:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor check "src/**/*.js" \\
    -x "//comment[contains(.,'TODO')]" \\
    --reason "No TODOs allowed" \\
    --expect-valid 'class Clean { }' \\
    --expect-invalid '// TODO: fix' \\
    -l javascript`}
      />
      <p>
        If the expectations fail (e.g. <code>--expect-valid</code> matches), the check reports an error for the rule itself.
      </p>

      <h2>Custom Messages</h2>
      <p>
        Use <code>-m</code> to customize the output message with template variables:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor check "src/**/*.js" \\
    -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO found" \\
    -m "{value}"`}
      />
      <p>
        Available variables: <code>{'{value}'}</code>, <code>{'{line}'}</code>, <code>{'{col}'}</code>, <code>{'{file}'}</code>.
      </p>

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td>XPath expression — each match is a violation</td></tr>
          <tr><td><code>--reason</code></td><td>Reason message for each violation</td></tr>
          <tr><td><code>--severity</code></td><td><code>error</code> (default) or <code>warning</code></td></tr>
          <tr><td><code>--rules</code></td><td>Path to a YAML/TOML rules file</td></tr>
          <tr><td><code>--expect-valid</code></td><td>Code example that should pass (no matches)</td></tr>
          <tr><td><code>--expect-invalid</code></td><td>Code example that should fail (has matches)</td></tr>
          <tr><td><code>-f, --format</code></td><td>Output format: gcc (default), github, text, json, yaml, xml, claude-code</td></tr>
          <tr><td><code>-m, --message</code></td><td>Custom message template</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/commands/test">test command</Link> — assert match counts against expectations.</p>
      </div>
    </DocLayout>
  );
}
