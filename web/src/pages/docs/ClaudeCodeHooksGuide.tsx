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

      <h3>The <code>--hook</code> parameter</h3>
      <p>
        The <code>-f claude-code</code> format supports a <code>--hook</code> parameter that controls the JSON envelope structure. Different Claude Code hook events expect different response formats:
      </p>
      <table className="doc-table">
        <thead>
          <tr><th><code>--hook</code> value</th><th>Behavior</th><th>Use case</th></tr>
        </thead>
        <tbody>
          <tr>
            <td><code>post-tool-use</code></td>
            <td>Blocks with <code>{`{ "decision": "block", "reason": "..." }`}</code></td>
            <td>Check files after edits (default)</td>
          </tr>
          <tr>
            <td><code>stop</code></td>
            <td>Same envelope as post-tool-use</td>
            <td>Final check before Claude stops</td>
          </tr>
          <tr>
            <td><code>pre-tool-use</code></td>
            <td>Denies with <code>{`{ "permissionDecision": "deny", ... }`}</code></td>
            <td>Prevent tool calls based on violations</td>
          </tr>
          <tr>
            <td><code>context</code></td>
            <td>Non-blocking: <code>{`{ "additionalContext": "..." }`}</code></td>
            <td>Feed violations as context without blocking</td>
          </tr>
        </tbody>
      </table>
      <p>
        When <code>--hook</code> is omitted, tractor defaults to <code>post-tool-use</code>. For the edit hook setup above, this is the right choice — it blocks Claude from proceeding until violations are fixed.
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

      <h2 id="other-hooks">Other hooks</h2>
      <ul>
        <li><Link to="/docs/guides/commit-hooks">Commit Hooks</Link> — run tractor as a git pre-commit hook for a local safety net alongside the edit hook</li>
        <li><Link to="/docs/guides/ci-cd#ai-generated-prs">CI/CD Integration</Link> — catch violations in AI-generated pull requests with inline annotations</li>
      </ul>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/use-cases">Use Cases</Link> — see the full range of ways teams use tractor.</p>
      </div>
    </DocLayout>
  );
}
