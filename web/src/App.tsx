import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Homepage } from './pages/Homepage';
import { Playground } from './pages/Playground';
import { DocsOverview } from './pages/docs/DocsOverview';
import { QueryCommand } from './pages/docs/QueryCommand';
import { CheckCommand } from './pages/docs/CheckCommand';
import { TestCommand } from './pages/docs/TestCommand';
import { RunCommand } from './pages/docs/RunCommand';
import { WritingQueries } from './pages/docs/WritingQueries';
import { SchemaGuide } from './pages/docs/SchemaGuide';
import { LintRulesGuide } from './pages/docs/LintRulesGuide';
import { CiCdGuide } from './pages/docs/CiCdGuide';

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Homepage />} />
        <Route path="/playground" element={<Playground />} />
        <Route path="/docs" element={<DocsOverview />} />
        <Route path="/docs/commands/query" element={<QueryCommand />} />
        <Route path="/docs/commands/check" element={<CheckCommand />} />
        <Route path="/docs/commands/test" element={<TestCommand />} />
        <Route path="/docs/commands/run" element={<RunCommand />} />
        <Route path="/docs/guides/writing-queries" element={<WritingQueries />} />
        <Route path="/docs/guides/schema" element={<SchemaGuide />} />
        <Route path="/docs/guides/lint-rules" element={<LintRulesGuide />} />
        <Route path="/docs/guides/ci-cd" element={<CiCdGuide />} />
      </Routes>
    </BrowserRouter>
  );
}
