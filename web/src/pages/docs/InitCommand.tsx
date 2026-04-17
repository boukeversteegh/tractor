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
        <code>tractor init</code> writes a minimal <code>tractor.yaml</code> to the current directory. The file opens with a short introduction so you know what it's for, and ships with a self-referential example rule: it scans <code>tractor.yaml</code> itself for <code>TODO:</code> markers, so running tractor straight away produces a visible result you can edit your way out of.
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
# The example rule below scans *this file* for reminder markers, so
# the TODO further down gets flagged the first time you run tractor.
# Edit \`files:\` to point at your own source, then replace the
# xpath/reason with the conventions you want to enforce.
#
# Full reference: https://tractor-cli.com/docs

check:
  files:
    - "tractor.yaml"
  # \`raw\` keeps YAML comments as queryable nodes — remove it once you
  # point \`files:\` at real source code.
  tree-mode: raw
  rules:
    - id: update-rules
      xpath: "//comment[contains(., 'TODO:')]"
      reason: "update this starter rule to match your project's conventions"
      severity: warning

# TODO: replace the example rule above with your own checks`}
      />

      <h2>Running the config</h2>
      <p>
        Because <code>tractor.yaml</code> sits in the current directory, <Link to="/docs/commands/run">tractor run</Link> picks it up automatically — no path argument needed:
      </p>
      <CodeBlock language="bash" code={`tractor run`} />
      <OutputBlock output={`tractor.yaml:26:1: warning: update this starter rule to match your project's conventions
26 | # TODO: replace the example rule above with your own checks
     ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
