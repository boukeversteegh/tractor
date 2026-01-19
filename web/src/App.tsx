import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Homepage } from './pages/Homepage';
import { Playground } from './pages/Playground';

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Homepage />} />
        <Route path="/playground" element={<Playground />} />
      </Routes>
    </BrowserRouter>
  );
}
