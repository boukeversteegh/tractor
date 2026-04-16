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
        Create <code>.git/hooks/pre-commit</code> (or add to your existing hook):
      </p>
      <CodeBlock
        language="bash"
        title=".git/hooks/pre-commit"
        code={`#!/bin/sh
# Run tractor on staged files only
STAGED=$(git diff --cached --name-only --diff-filter=ACM)

if [ -n "$STAGED" ]; then
  tractor run .tractor.yml --files $STAGED
fi`}
      />
      <p>
        Make it executable with <code>chmod +x .git/hooks/pre-commit</code>.
      </p>

      <h2 id="pre-commit-framework">Using pre-commit framework</h2>
      <p>
        If you use the <a href="https://pre-commit.com" target="_blank" rel="noopener noreferrer">pre-commit</a> framework, add tractor as a local hook:
      </p>
      <CodeBlock
        language="yaml"
        title=".pre-commit-config.yaml"
        code={`repos:
  - repo: local
    hooks:
      - id: tractor
        name: tractor
        entry: tractor run .tractor.yml --files
        language: system
        pass_filenames: true`}
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
