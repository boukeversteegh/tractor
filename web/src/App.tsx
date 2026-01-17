import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { initParser, parseSource } from './parser';
import { initTractor, parseAstToXmlSimple, validateXPathSync, XPathValidationResult } from './tractor';
import { queryXml, Match, OutputFormat, OUTPUT_FORMATS } from './xpath';
import { TreeView } from './components/TreeView';
import { XmlOutput } from './components/XmlOutput';
import { QueryResults } from './components/QueryResults';
import { SourceEditor } from './components/SourceEditor';
import { QueryInput } from './components/QueryInput';
import { Tabs } from './components/Tabs';
import { SAMPLE_CODE } from './sampleCode';
import {
  parseXmlToTree,
  XmlNode,
  findDeepestNodeAtPosition,
  getPathToNode,
  offsetToPosition,
} from './xmlTree';
import {
  SelectionState,
  createEmptyState,
  clearSelection,
} from './queryState';

type Tab = 'builder' | 'xml';

// Simple query builder from selection state
// Maps nodeId -> { name, selected, isTarget, condition }
interface NodeInfo {
  name: string;
  selected: boolean;
  isTarget: boolean;
  condition?: string;
}

function buildQueryFromSelection(
  selectionState: SelectionState,
  nodeInfoMap: Map<string, NodeInfo>,
  tree: XmlNode | null
): string {
  const selected: { id: string; info: NodeInfo; depth: number }[] = [];

  for (const [id, state] of selectionState) {
    const info = nodeInfoMap.get(id);
    if (!info) continue;

    if (state.selected) {
      // Get depth from tree path
      const path = tree ? getPathToNode(tree, id) : null;
      const depth = path ? path.length : 0;
      selected.push({ id, info: { ...info, ...state }, depth });
    }
  }

  if (selected.length === 0) return '';

  // Sort by tree depth (ancestors first)
  selected.sort((a, b) => a.depth - b.depth);

  const names = selected.map(s => {
    let part = s.info.name;
    if (s.info.condition) {
      part += `[${s.info.condition}]`;
    }
    return part;
  });

  return '//' + names.join('//');
}

export function App() {
  const [initialized, setInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Source & parsing state
  const [source, setSource] = useState(SAMPLE_CODE.csharp);
  const [language, setLanguage] = useState('csharp');
  const [xml, setXml] = useState('');  // Display XML (may not have locations)
  const [xmlForQuery, setXmlForQuery] = useState('');  // XML with locations for querying
  const [xmlTree, setXmlTree] = useState<XmlNode | null>(null);

  // Options
  const [rawMode, setRawMode] = useState(false);
  const [showLocations, setShowLocations] = useState(false);
  const [prettyPrint, setPrettyPrint] = useState(true);

  // Selection state
  const [selectionState, setSelectionState] = useState<SelectionState>(createEmptyState);
  const [nodeInfoMap, setNodeInfoMap] = useState<Map<string, NodeInfo>>(new Map());

  // Derived XPath query from selection
  const query = useMemo(() => {
    return buildQueryFromSelection(selectionState, nodeInfoMap, xmlTree);
  }, [selectionState, nodeInfoMap, xmlTree]);

  // Manual query override
  const [manualQuery, setManualQuery] = useState('');
  const [useManualQuery, setUseManualQuery] = useState(false);

  const effectiveQuery = useManualQuery ? manualQuery : query;

  // Query results
  const [matches, setMatches] = useState<Match[]>([]);
  const [queryValidation, setQueryValidation] = useState<XPathValidationResult | null>(null);
  const [outputFormat, setOutputFormat] = useState<OutputFormat>('source');

  // UI state
  const [activeTab, setActiveTab] = useState<Tab>('builder');
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(new Set());

  // Panel resize state
  const STORAGE_KEY = 'tractor-panel-widths';
  const defaultWidths = [33, 34, 33];
  const [panelWidths, setPanelWidths] = useState<number[]>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored);
        if (Array.isArray(parsed) && parsed.length === 3) return parsed;
      }
    } catch {}
    return defaultWidths;
  });

  const containerRef = useRef<HTMLElement>(null);
  const dragRef = useRef<{ index: number; startX: number; startWidths: number[] } | null>(null);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(panelWidths));
  }, [panelWidths]);

  const handleResizeStart = useCallback((index: number, e: React.MouseEvent) => {
    e.preventDefault();
    dragRef.current = { index, startX: e.clientX, startWidths: [...panelWidths] };

    const handleMouseMove = (e: MouseEvent) => {
      if (!dragRef.current || !containerRef.current) return;
      const { index, startX, startWidths } = dragRef.current;
      const containerWidth = containerRef.current.offsetWidth;
      const deltaPercent = ((e.clientX - startX) / containerWidth) * 100;

      const newWidths = [...startWidths];
      const minWidth = 10;

      newWidths[index] = Math.max(minWidth, startWidths[index] + deltaPercent);
      newWidths[index + 1] = Math.max(minWidth, startWidths[index + 1] - deltaPercent);

      // Clamp to ensure both panels stay above minimum
      if (newWidths[index] < minWidth) {
        newWidths[index] = minWidth;
        newWidths[index + 1] = startWidths[index] + startWidths[index + 1] - minWidth;
      }
      if (newWidths[index + 1] < minWidth) {
        newWidths[index + 1] = minWidth;
        newWidths[index] = startWidths[index] + startWidths[index + 1] - minWidth;
      }

      setPanelWidths(newWidths);
    };

    const handleMouseUp = () => {
      dragRef.current = null;
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }, [panelWidths]);

  // Initialize parsers
  useEffect(() => {
    async function init() {
      try {
        await Promise.all([initParser(), initTractor()]);
        setInitialized(true);
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to initialize');
      }
    }
    init();
  }, []);

  // Parse source when it changes
  useEffect(() => {
    if (!initialized || !source.trim()) {
      setXml('');
      setXmlTree(null);
      return;
    }

    async function parse() {
      try {
        const ast = await parseSource(source, language);

        // Always parse with locations for tree building and querying
        // Use non-pretty-printed XML for querying to avoid whitespace in textContent
        const xmlWithLocations = await parseAstToXmlSimple(
          ast, source, language, rawMode, true, false  // never pretty-print for queries
        );
        setXmlForQuery(xmlWithLocations);  // Store for querying (has locations, no whitespace)

        // Parse XML to tree for query builder (needs locations)
        const tree = parseXmlToTree(xmlWithLocations);
        setXmlTree(tree);

        // Generate display XML based on user preference
        const xmlOutput = showLocations
          ? xmlWithLocations
          : await parseAstToXmlSimple(ast, source, language, rawMode, false, prettyPrint);
        setXml(xmlOutput);  // For display only

        // Build node info map
        if (tree) {
          const infoMap = new Map<string, NodeInfo>();
          function traverse(node: XmlNode) {
            infoMap.set(node.id, {
              name: node.name,
              selected: false,
              isTarget: false,
            });
            for (const child of node.children) {
              traverse(child);
            }
          }
          traverse(tree);
          setNodeInfoMap(infoMap);
        }
      } catch (e) {
        console.error('Parse error:', e);
        setXml('');
        setXmlTree(null);
      }
    }

    const timeout = setTimeout(parse, 300);
    return () => clearTimeout(timeout);
  }, [initialized, source, language, rawMode, showLocations, prettyPrint]);

  // Clear selection when tree changes
  useEffect(() => {
    setSelectionState(createEmptyState());
    setUseManualQuery(false);
  }, [xmlTree]);

  // Execute XPath query (use xmlForQuery which has locations)
  useEffect(() => {
    if (!effectiveQuery.trim()) {
      setMatches([]);
      setQueryValidation(null);
      return;
    }

    // Validate XPath first
    const validation = validateXPathSync(effectiveQuery);
    setQueryValidation(validation);

    if (!validation.valid) {
      setMatches([]);
      return;
    }

    if (!xmlForQuery) {
      setMatches([]);
      return;
    }

    try {
      const results = queryXml(xmlForQuery, effectiveQuery);
      setMatches(results);
    } catch (e) {
      console.error('XPath error:', e);
      setQueryValidation({
        valid: false,
        error: e instanceof Error ? e.message : 'Query execution failed',
        warnings: [],
      });
      setMatches([]);
    }
  }, [xmlForQuery, effectiveQuery]);

  // Handle language change
  const handleLanguageChange = useCallback((newLang: string) => {
    setLanguage(newLang);
    if (SAMPLE_CODE[newLang]) {
      setSource(SAMPLE_CODE[newLang]);
    }
  }, []);

  // Selection handlers
  const handleToggleSelection = useCallback((nodeId: string, nodeName: string) => {
    setSelectionState(prev => {
      const newState = new Map(prev);
      const current = newState.get(nodeId);

      if (current?.selected) {
        newState.delete(nodeId);
      } else {
        newState.set(nodeId, {
          selected: true,
          isTarget: current?.isTarget ?? false,
          condition: current?.condition,
        });
      }
      return newState;
    });
    setNodeInfoMap(prev => {
      const newMap = new Map(prev);
      const info = newMap.get(nodeId);
      if (info) {
        newMap.set(nodeId, { ...info, name: nodeName });
      }
      return newMap;
    });
    setUseManualQuery(false);
  }, []);

  const handleSetTarget = useCallback((nodeId: string, nodeName: string) => {
    setSelectionState(prev => {
      const newState = new Map(prev);

      // Clear previous target
      for (const [id, state] of newState) {
        if (state.isTarget) {
          newState.set(id, { ...state, isTarget: false });
        }
      }

      // Set new target (also select it)
      const current = newState.get(nodeId);
      newState.set(nodeId, {
        selected: true,
        isTarget: true,
        condition: current?.condition,
      });

      return newState;
    });
    setNodeInfoMap(prev => {
      const newMap = new Map(prev);
      const info = newMap.get(nodeId);
      if (info) {
        newMap.set(nodeId, { ...info, name: nodeName });
      }
      return newMap;
    });
    setUseManualQuery(false);
  }, []);

  const handleAddCondition = useCallback((nodeId: string, condition: string) => {
    setSelectionState(prev => {
      const newState = new Map(prev);
      const current = newState.get(nodeId);
      newState.set(nodeId, {
        selected: current?.selected ?? true,
        isTarget: current?.isTarget ?? false,
        condition,
      });
      return newState;
    });
    setUseManualQuery(false);
  }, []);

  const handleClearSelection = useCallback(() => {
    setSelectionState(clearSelection());
    setUseManualQuery(false);
  }, []);

  const handleQueryChange = useCallback((newQuery: string) => {
    setManualQuery(newQuery);
    setUseManualQuery(true);
  }, []);

  // Handle click on source code to focus corresponding tree node
  const handleSourceClick = useCallback((e: React.MouseEvent<HTMLTextAreaElement>) => {
    if (!xmlTree) return;

    const textarea = e.currentTarget;
    const cursorOffset = textarea.selectionStart;
    const position = offsetToPosition(source, cursorOffset);

    const node = findDeepestNodeAtPosition(xmlTree, position);

    if (node) {
      setFocusedNodeId(node.id);

      // Expand all ancestors of the focused node
      const path = getPathToNode(xmlTree, node.id);
      if (path) {
        setExpandedNodeIds(prev => {
          const newSet = new Set(prev);
          for (const id of path) {
            newSet.add(id);
          }
          return newSet;
        });
      }

      // Switch to builder tab if not already there
      setActiveTab('builder');
    }
  }, [xmlTree, source]);

  if (error) {
    return (
      <div className="app error-screen">
        <h1>Failed to initialize</h1>
        <p>{error}</p>
      </div>
    );
  }

  if (!initialized) {
    return (
      <div className="app loading-screen">
        <div className="spinner"></div>
        <p>Loading TreeSitter grammars...</p>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="app-header">
        <div className="header-left">
          <h1>Tractor</h1>
          <span className="subtitle">XPath for Code</span>
        </div>
        <div className="query-bar">
          <label>XPath:</label>
          <QueryInput
            value={effectiveQuery}
            onChange={handleQueryChange}
            placeholder="Click nodes to build query..."
            className={`${useManualQuery ? 'manual' : ''} ${queryValidation && !queryValidation.valid ? 'error' : ''}`}
            errorStart={queryValidation && !queryValidation.valid ? queryValidation.error_start : undefined}
            errorEnd={queryValidation && !queryValidation.valid ? queryValidation.error_end : undefined}
          />
          {queryValidation && !queryValidation.valid ? (
            <span className="query-error" title={queryValidation.error}>
              ⚠ {(queryValidation.error?.length ?? 0) > 30 ? queryValidation.error?.slice(0, 30) + '...' : queryValidation.error}
            </span>
          ) : (
            <span className="match-count">
              {matches.length} match{matches.length !== 1 ? 'es' : ''}
            </span>
          )}
          <button onClick={handleClearSelection} className="clear-btn">Clear</button>
        </div>
      </header>

      <main className="app-main" ref={containerRef}>
        <div className="panel source-panel" style={{ width: `${panelWidths[0]}%` }}>
          <div className="panel-header">
            <span>Source</span>
            <select value={language} onChange={(e) => handleLanguageChange(e.target.value)}>
              <option value="typescript">TypeScript</option>
              <option value="javascript">JavaScript</option>
              <option value="csharp">C#</option>
              <option value="rust">Rust</option>
              <option value="python">Python</option>
              <option value="go">Go</option>
              <option value="java">Java</option>
              <option value="ruby">Ruby</option>
              <option value="cpp">C++</option>
              <option value="c">C</option>
              <option value="json">JSON</option>
              <option value="html">HTML</option>
              <option value="css">CSS</option>
              <option value="bash">Bash</option>
              <option value="php">PHP</option>
            </select>
          </div>
          <SourceEditor
            source={source}
            matches={matches}
            onChange={setSource}
            onClick={handleSourceClick}
          />
        </div>

        <div className="resize-handle" onMouseDown={(e) => handleResizeStart(0, e)} />

        <div className="panel output-panel" style={{ width: `${panelWidths[1]}%` }}>
          <div className="panel-header">
            <span>Structure</span>
            <Tabs
              tabs={[
                { value: 'builder' as Tab, label: 'Builder' },
                { value: 'xml' as Tab, label: 'XML' },
              ]}
              value={activeTab}
              onChange={setActiveTab}
            />
            {activeTab === 'xml' && (
              <div className="options">
                <label>
                  <input type="checkbox" checked={rawMode} onChange={(e) => setRawMode(e.target.checked)} />
                  Raw
                </label>
                <label>
                  <input type="checkbox" checked={showLocations} onChange={(e) => setShowLocations(e.target.checked)} />
                  Locations
                </label>
                <label>
                  <input type="checkbox" checked={prettyPrint} onChange={(e) => setPrettyPrint(e.target.checked)} />
                  Pretty
                </label>
              </div>
            )}
          </div>
          <div className="tab-content">
            {activeTab === 'builder' ? (
              <TreeView
                xmlTree={xmlTree}
                selectionState={selectionState}
                focusedNodeId={focusedNodeId}
                expandedNodeIds={expandedNodeIds}
                onToggleSelection={handleToggleSelection}
                onSetTarget={handleSetTarget}
                onAddCondition={handleAddCondition}
                onExpandedChange={setExpandedNodeIds}
              />
            ) : (
              <XmlOutput xml={xml} />
            )}
          </div>
        </div>

        <div className="resize-handle" onMouseDown={(e) => handleResizeStart(1, e)} />

        <div className="panel results-panel" style={{ width: `${panelWidths[2]}%` }}>
          <div className="panel-header">
            <span>Results</span>
            <Tabs
              tabs={OUTPUT_FORMATS.map((f) => ({
                value: f.value,
                label: f.label,
                title: f.description,
              }))}
              value={outputFormat}
              onChange={setOutputFormat}
            />
          </div>
          <QueryResults matches={matches} format={outputFormat} source={source} />
        </div>
      </main>
    </div>
  );
}
