import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const APP_JS = `// TODO: fix this later
class App {
  run() { }
}`;

export function LintRulesGuide() {
  return (
    <DocLayout>
      <h1>Writing Lint Rules</h1>
      <p className="doc-lead">
        Turn your team's conventions into enforceable rules. Start with a query, add a reason, and run it in CI.
      </p>

      <h2>From Query to Rule</h2>
      <p>
        Every lint rule starts as a query. If the query matches, it's a violation. Here's the progression:
      </p>

      <h3>1. Find the pattern</h3>
      <Example
        file={{ name: 'app.js', language: 'js', content: APP_JS }}
        command={`tractor app.js -x "//comment[contains(.,'TODO')]" -v value`}
        output="// TODO: fix this later"
      />

      <h3>2. Turn it into a check</h3>
      <Example
        command={`tractor check app.js -x "//comment[contains(.,'TODO')]" \\
    --reason "TODO comments should be resolved"`}
        output={`app.js:1:1: error: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~


1 error in 1 file`}
      />

      <h3>3. Add it to a config file</h3>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: error`}
      />

      <h2>Rule Examples</h2>

      <h3>No TODO comments</h3>
      <CodeBlock
        language="yaml"
        code={`- id: no-todo
  xpath: "//comment[contains(.,'TODO')]"
  reason: "TODO comments should be resolved"
  severity: warning`}
      />

      <h3>Repository methods must use orderBy</h3>
      <CodeBlock
        language="yaml"
        code={`- id: repository-needs-orderby
  xpath: >-
    //class[contains(name,'Repository')]
    //method[contains(name,'getAll')]
    [not(contains(.,'orderBy'))]/name
  reason: "getAll methods in repositories should use orderBy"
  severity: error`}
      />

      <h3>Functions should not have too many parameters</h3>
      <CodeBlock
        language="yaml"
        code={`- id: max-parameters
  xpath: "//function[count(parameters/params/type) > 5]/name"
  reason: "Functions should have at most 5 parameters"
  severity: warning`}
      />

      <h2>Testing Your Rules</h2>
      <p>
        Add <code>expect</code> blocks to verify your rules work correctly:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
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
        <code>valid</code> examples should <strong>not</strong> trigger the rule. <code>invalid</code> examples <strong>should</strong> trigger it. If either fails, the run reports it as an error.
      </p>
      <p>
        You can also test inline from the command line:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor check "src/**/*.js" \\
    -x "//comment[contains(.,'TODO')]" \\
    --reason "No TODOs" \\
    --expect-valid 'class Clean { }' \\
    --expect-invalid '// TODO: fix' \\
    -l javascript`}
      />

      <h2>Severity Levels</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Level</th><th>Exit code</th><th>Use case</th></tr>
        </thead>
        <tbody>
          <tr><td><code>error</code></td><td>1 (fails build)</td><td>Must-fix violations</td></tr>
          <tr><td><code>warning</code></td><td>0 (passes)</td><td>Suggestions, gradual adoption</td></tr>
        </tbody>
      </table>

      <h2>Gradual Adoption</h2>
      <p>
        When introducing rules to an existing codebase:
      </p>
      <ol>
        <li>Start with <code>severity: warning</code> so the build doesn't break</li>
        <li>Fix existing violations over time</li>
        <li>Switch to <code>severity: error</code> once the codebase is clean</li>
        <li>Use <code>--diff-lines</code> to only check new code</li>
      </ol>

      <CodeBlock
        language="yaml"
        title="Only check new code"
        code={`diff-lines: "main..HEAD"

check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: error`}
      />

      <h2>Multi-Language Rules</h2>
      <p>
        Since tractor auto-detects languages by file extension, one config can cover multiple languages:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`operations:
  - check:
      files:
        - "src/**/*.js"
      rules:
        - id: no-todo-js
          xpath: "//comment[contains(.,'TODO')]"
          reason: "TODO comment in JavaScript"
          severity: warning

  - check:
      files:
        - "src/**/*.rs"
      rules:
        - id: no-todo-rs
          xpath: "//line_comment[contains(.,'TODO')]"
          reason: "TODO comment in Rust"
          severity: warning`}
      />

      <h2>Custom Messages</h2>
      <p>
        Use the <code>message</code> property with template variables to customize output:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: no-todo
  xpath: "//comment[contains(.,'TODO')]"
  reason: "TODO comments should be resolved"
  message: "{value}"
  severity: warning`}
      />
      <p>
        Available variables: <code>{'{value}'}</code>, <code>{'{line}'}</code>, <code>{'{col}'}</code>, <code>{'{file}'}</code>.
      </p>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/ci-cd">CI/CD Integration</Link> — set up tractor in your pipeline.</p>
      </div>
    </DocLayout>
  );
}
