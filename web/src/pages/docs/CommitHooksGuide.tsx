import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock } from '../../components/CodeBlock';

export function CommitHooksGuide() {
  return (
    <DocLayout>
      <h1>Commit Hooks</h1>
      <p className="doc-lead">
        Run tractor as a git pre-commit hook to catch violations before they're committed — whether the code was written by a human or an AI agent.
      </p>

      <h2 id="how-it-works">How it works</h2>
      <p>
        Git pre-commit hooks run automatically before every <code>git commit</code>. By wiring tractor into this hook, you get a local safety net that checks only the staged files — fast even on large codebases. If tractor finds errors, the commit is blocked and you see the violations immediately.
      </p>

      <h2 id="shell-script">Using a shell script</h2>
      <p>
        Create <code>.git/hooks/pre-commit</code> (or add to your existing hook). Tractor's built-in <code>--diff-files</code> flag handles scoping to staged files — no need to manually list them with <code>git diff</code>:
      </p>
      <CodeBlock
        language="bash"
        title=".git/hooks/pre-commit"
        code={`#!/bin/sh
tractor run .tractor.yml --diff-files "--cached"`}
      />
      <p>
        Make it executable with <code>chmod +x .git/hooks/pre-commit</code>.
      </p>
      <p>
        The <code>--diff-files "--cached"</code> flag tells tractor to only check files that are staged for commit. Keep this on the command line rather than in your <code>.tractor.yml</code> — that way your rule file stays reusable for CI, editor hooks, and other contexts.
      </p>

      <h2 id="pre-commit-framework">Using pre-commit framework</h2>
      <p>
        If you use the <a href="https://pre-commit.com" target="_blank" rel="noopener noreferrer">pre-commit</a> framework, you can either use <code>--diff-files</code> or let the framework pass filenames directly:
      </p>
      <CodeBlock
        language="yaml"
        title=".pre-commit-config.yaml"
        code={`repos:
  - repo: local
    hooks:
      - id: tractor
        name: tractor
        entry: tractor run .tractor.yml --diff-files "--cached"
        language: system
        pass_filenames: false`}
      />

      <h2 id="example-config">Example rule file</h2>
      <p>
        A typical <code>.tractor.yml</code> for commit hooks:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "src/**/*.cs"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "Resolve TODO comments before committing"
      severity: error

    - id: method-pascalcase
      xpath: "//method[public]/name[matches(., '^[a-z]')]"
      reason: "Public methods should use PascalCase"
      severity: error`}
      />

      <h2 id="tips">Tips</h2>
      <ul>
        <li>The hook only checks staged files (<code>--diff-filter=ACM</code> skips deletions), so it stays fast</li>
        <li>Use <code>git commit --no-verify</code> to skip the hook when needed (e.g. work-in-progress commits)</li>
        <li>Combine with <Link to="/docs/guides/ci-cd">CI/CD checks</Link> for defense in depth — the commit hook catches issues locally, CI catches anything that slips through</li>
        <li>For AI-assisted development, pair with <Link to="/docs/guides/claude-code-hooks">Claude Code hooks</Link> for real-time feedback during editing</li>
      </ul>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/ci-cd">CI/CD Integration</Link> — enforce conventions in your pipeline.</p>
      </div>
    </DocLayout>
  );
}
