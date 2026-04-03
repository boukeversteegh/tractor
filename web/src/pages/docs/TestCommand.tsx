import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

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
      <CodeBlock
        language="bash"
        code={`echo 'public class Foo { }\npublic class Bar { }' | tractor test -l csharp \\
    -x "//class" --expect 2 -m "Expected 2 classes"`}
      />
      <OutputBlock output={`✓ Expected 2 classes`} />

      <h2>Expect None</h2>
      <p>
        Assert that a pattern does <em>not</em> appear:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Foo { }' | tractor test -l csharp \\
    -x "//interface" --expect none -m "No interfaces expected"`}
      />
      <OutputBlock output={`✓ No interfaces expected`} />

      <h2>Expect Some</h2>
      <p>
        Assert that at least one match exists:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Foo { }' | tractor test -l csharp \\
    -x "//class" --expect some -m "At least one class expected"`}
      />
      <OutputBlock output={`✓ At least one class expected`} />

      <h2>Failed Assertions</h2>
      <p>
        When the expectation is not met, tractor reports the failure and exits with code <code>1</code>:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Foo { }' | tractor test -l csharp \\
    -x "//class" --expect none -m "No classes expected"`}
      />
      <OutputBlock output={`✗ No classes expected (expected none, got 1)`} />

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
          <tr><td><code>-f, --format</code></td><td>Output format: text (default), json, yaml, gcc, github</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/commands/run">run command</Link> — execute a config file with multiple operations.</p>
      </div>
    </DocLayout>
  );
}
