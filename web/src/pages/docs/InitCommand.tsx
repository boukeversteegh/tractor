import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

export function InitCommand() {
  return (
    <DocLayout>
      <h1>init</h1>
      <p className="doc-lead">
        Scaffold a <code>tractor.yaml</code> in the current directory so you can get started without writing config by hand.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor init [--force]`} language="bash" />

      <h2>What it does</h2>
      <p>
        <code>tractor init</code> writes a minimal <code>tractor.yaml</code> to the current directory. The starter file contains a single <code>check</code> rule that flags <code>TODO</code> comments in any file — a concrete example that's easy to recognize, edit, or replace with your own rules.
      </p>
      <CodeBlock language="bash" code={`tractor init`} />
      <OutputBlock output={`created tractor.yaml
run \`tractor run\` to execute it`} />

      <h3>Generated file</h3>
      <CodeBlock
        language="yaml"
        title="tractor.yaml"
        code={`check:
  files:
    - "**/*"
  rules:
    - id: no-todo
      xpath: "//comment[contains(., 'TODO')]"
      reason: "TODO comment found"
      severity: warning`}
      />

      <h2>Running the config</h2>
      <p>
        Because <code>tractor.yaml</code> sits in the current directory, <Link to="/docs/commands/run">tractor run</Link> picks it up automatically — no path argument needed:
      </p>
      <CodeBlock language="bash" code={`tractor run`} />

      <h2>Safety</h2>
      <p>
        If a <code>tractor.yaml</code> already exists, <code>init</code> refuses to overwrite it and exits with an error. Pass <code>--force</code> to replace the file with the starter template.
      </p>
      <CodeBlock language="bash" code={`tractor init --force`} />

      <div className="doc-next">
        <p>Next: <Link to="/docs/commands/run">run command</Link> — execute rules from your config file.</p>
      </div>
    </DocLayout>
  );
}
