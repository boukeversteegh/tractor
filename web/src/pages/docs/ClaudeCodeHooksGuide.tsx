import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock } from '../../components/CodeBlock';

export function ClaudeCodeHooksGuide() {
  return (
    <DocLayout>
      <h1>Claude Code Hooks</h1>
      <p className="doc-lead">
        Run tractor automatically every time Claude edits a file — catching violations the moment they're introduced, not after a full PR review cycle.
      </p>

      <h2 id="how-it-works">How it works</h2>
      <p>
        Claude Code supports <strong>hooks</strong> — shell commands that run in response to tool events. By wiring tractor into a <code>PostToolUse</code> hook, you get instant feedback every time Claude creates or modifies a file. Claude sees the violations as error messages and fixes them immediately, just like it would with a compiler error or linter warning.
      </p>

      <h2 id="setup">Setting up the edit hook</h2>
      <p>
        Add this to your project's <code>.claude/settings.json</code>:
      </p>
      <CodeBlock
        language="json"
        title=".claude/settings.json"
        code={`{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "FILE=$(jq -r '.tool_input.file_path') && tractor run .tractor.yml --files \\"$FILE\\" -f claude-code"
          }
        ]
      }
    ]
  }
}`}
      />
      <p>
        The hook triggers after every <code>Edit</code> or <code>Write</code> tool call. It reads the file path from the tool input, then runs tractor against just that file using your project's rule file.
      </p>
      <p>
        <strong>Note:</strong> This hook currently requires <a href="https://jqlang.github.io/jq/" target="_blank" rel="noopener noreferrer">jq</a> to be installed (used to extract the file path from the hook payload). We're working on making this integration seamless without external dependencies.
      </p>

      <h3>Output format</h3>
      <p>
        The <code>-f claude-code</code> flag tells tractor to emit the JSON format that Claude Code hooks expect — no extra <code>jq</code> wrapping needed. When violations are found, Claude sees the errors and fixes them immediately. When the file is clean, tractor outputs nothing and the hook passes silently.
      </p>

      <h2 id="example-rules">Example rule file</h2>
      <p>
        Here's a <code>.tractor.yml</code> that enforces common conventions:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "src/**/*.cs"
  rules:
    - id: authorize-required
      xpath: >-
        //class[contains(name,'Controller')]
        /method[public]
        [not(attrs[contains(.,'Authorize')])]
        [not(attrs[contains(.,'AllowAnonymous')])]/name
      reason: "Controller actions must have [Authorize] or [AllowAnonymous]"
      severity: error

    - id: tenant-id-required
      xpath: >-
        //class[contains(name,'Controller')]
        /method[public]
        [not(contains(.,'GetTenantId'))]/name
      reason: "Controller actions must call GetTenantId()"
      severity: error

    - id: method-pascalcase
      xpath: "//method[public]/name[matches(., '^[a-z]')]"
      reason: "Public methods should use PascalCase"
      severity: error`}
      />

      <h2 id="pre-commit-hook">Pre-commit hook</h2>
      <p>
        You can also run tractor as a <strong>git pre-commit hook</strong> to catch violations before they're committed — whether the code was written by a human or an AI agent. This is useful as a local safety net alongside the Claude Code edit hook.
      </p>

      <h3>Using a shell script</h3>
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

      <h3>Using pre-commit framework</h3>
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
      <p>
        Both approaches only check staged files, so the hook stays fast even on large codebases. If tractor finds errors, the commit is blocked and you see the violations immediately.
      </p>

      <h2 id="ci-integration">CI for AI-generated PRs</h2>
      <p>
        For AI agents that work asynchronously — opening PRs rather than editing interactively — add tractor to your CI pipeline. With <code>--diff-lines</code> and <code>-f github</code>, violations appear as inline annotations on the PR that the AI can read and fix automatically. See the <Link to="/docs/guides/ci-cd#ai-generated-prs">CI/CD Integration guide</Link> for full setup instructions.
      </p>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/use-cases">Use Cases</Link> — see the full range of ways teams use tractor.</p>
      </div>
    </DocLayout>
  );
}
