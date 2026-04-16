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
        Claude Code supports <strong>hooks</strong> — shell commands that run in response to tool events. Tractor currently integrates via the <code>PostToolUse</code> hook, which fires <em>after</em> an edit has been written to disk. This means violations don't prevent the file from being updated — instead, Claude sees the errors immediately after the edit and fixes them in a follow-up change.
      </p>
      <p>
        This gives you a fast correct-and-fix loop: Claude writes code, tractor checks it, Claude fixes any issues. We're working on a pre-edit integration that will prevent violations from being written in the first place.
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
            "command": "FILE=$(jq -r '.tool_input.file_path') && tractor run tractor.yml --files \\"$FILE\\" -f claude-code"
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

      <p>
        The hook references a <code>tractor.yml</code> config file that defines your rules. See the <Link to="/docs/guides/lint-rules">Writing Lint Rules</Link> guide and <Link to="/docs/guides/use-cases">Use Cases</Link> for examples of what to put in it.
      </p>

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
