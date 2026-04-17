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
        <code>tractor init</code> writes a minimal <code>tractor.yaml</code> to the current directory. The file opens with a short introduction, and ships with a self-referential sample rule: the xpath is the full path to the rule itself, filtered by its id. When you run tractor, the whole rule block lights up — a live demonstration of how a rule maps onto the YAML tree.
      </p>
      <CodeBlock language="bash" code={`tractor init`} />
      <OutputBlock output={`created tractor.yaml
run \`tractor run\` to execute it`} />

      <h3>Generated file</h3>
      <CodeBlock
        language="yaml"
        title="tractor.yaml"
        code={`# Tractor config
# ---------------
# This file declares checks that tractor runs against your project.
# Run \`tractor run\` from this directory — tractor picks up
# \`tractor.yaml\` automatically when it sits next to you.
#
# The sample rule below points at *this file* and matches itself by
# rule id. When you run tractor, the whole rule block gets flagged —
# a live demonstration of how rules are structured. Edit \`files:\` to
# point at your own source, then rewrite the rule to enforce the
# conventions you care about.
#
# Full reference: https://tractor-cli.com/docs

check:
  files:
    - "tractor.yaml"
  rules:
    - id: sample-rule
      xpath: "/stream/document/check/rules[id='sample-rule']"
      reason: "replace this sample rule with your own checks"
      severity: warning`}
      />

      <h2>Running the config</h2>
      <p>
        Because <code>tractor.yaml</code> sits in the current directory, <Link to="/docs/commands/run">tractor run</Link> picks it up automatically — no path argument needed:
      </p>
      <CodeBlock language="bash" code={`tractor run`} />
      <OutputBlock output={`tractor.yaml:19:7: warning: replace this sample rule with your own checks
19 >|     - id: sample-rule
20  |       xpath: "/stream/document/check/rules[id='sample-rule']"
21  |       reason: "replace this sample rule with your own checks"
22 >|       severity: warning

1 warning in 1 file`} />

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
