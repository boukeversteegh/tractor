import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

export function TestCommand() {
  return (
    <DocLayout>
      <h1>test</h1>
      <p className="doc-lead">
        Assert match counts against expectations. Useful for verifying structural properties of your codebase.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor test [FILES] -x <XPATH> --expect <EXPECT> [OPTIONS]`} language="bash" />

      <h2>Basic Test</h2>
      <p>
        Assert that a query matches an exact number of times:
      </p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command={`tractor test greeter.js -x "//function" --expect 2 \\
    -m "Expected 2 functions"`}
        output={`✓ Expected 2 functions`}
      />

      <h2>Expect None</h2>
      <p>
        Assert that a pattern does <em>not</em> appear:
      </p>
      <Example
        command={`tractor test greeter.js -x "//class" --expect none \\
    -m "No classes expected"`}
        output={`✓ No classes expected`}
      />

      <h2>Expect Some</h2>
      <p>
        Assert that at least one match exists:
      </p>
      <Example
        command={`tractor test greeter.js -x "//function" --expect some \\
    -m "At least one function"`}
        output={`✓ At least one function`}
      />

      <h2>Failed Assertions</h2>
      <p>
        When the expectation is not met, tractor reports the failure and exits with code <code>1</code>:
      </p>
      <Example
        command={`tractor test greeter.js -x "//function" --expect none \\
    -m "No functions expected"`}
        output={`✗ No functions expected (expected none, got 2)`}
      />

      <h2>Use Cases</h2>
      <ul>
        <li>Verify that migration files have a specific structure</li>
        <li>Assert that a module exports exactly N public functions</li>
        <li>Check that no file exceeds a certain number of classes</li>
        <li>Validate that configuration files contain required sections</li>
      </ul>

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td>XPath expression to match</td></tr>
          <tr><td><code>-e, --expect</code></td><td>Expected result: <code>none</code>, <code>some</code>, or a number</td></tr>
          <tr><td><code>-m, --message</code></td><td>Custom message for the assertion</td></tr>
          <tr><td><code>-s, --string</code></td><td>Inline source code to test</td></tr>
          <tr><td><code>-l, --lang</code></td><td>Language for stdin/string input</td></tr>
          <tr><td><code>-f, --format</code></td><td>Output format: text (default), json, yaml, gcc, github, claude-code</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/commands/run">run command</Link> — execute a config file with multiple operations.</p>
      </div>
    </DocLayout>
  );
}
