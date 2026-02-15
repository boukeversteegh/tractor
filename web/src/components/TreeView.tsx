import { useState, useCallback, useRef, useEffect } from 'react';
import { SchemaNode } from '../tractor';
import { SelectionState } from '../queryState';

interface TreeViewProps {
  schemaTree: SchemaNode[];
  selectionState: SelectionState;
  effectiveTargetKey: string | null;
  focusedPathKey: string | null;
  expandedPaths: Set<string>;
  onToggleSelection: (pathKey: string, nodeName: string) => void;
  onSetTarget: (pathKey: string, nodeName: string) => void;
  onAddCondition: (pathKey: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
}

export function TreeView({
  schemaTree,
  selectionState,
  effectiveTargetKey,
  focusedPathKey,
  expandedPaths,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
}: TreeViewProps) {
  if (schemaTree.length === 0) {
    return <div className="tree-view empty">No tree to display</div>;
  }

  return (
    <div className="tree-view">
      {schemaTree.map((node) => (
        <SchemaTreeNode
          key={node.name}
          node={node}
          ancestorPath={[]}
          depth={0}
          selectionState={selectionState}
          effectiveTargetKey={effectiveTargetKey}
          focusedPathKey={focusedPathKey}
          expandedPaths={expandedPaths}
          onToggleSelection={onToggleSelection}
          onSetTarget={onSetTarget}
          onAddCondition={onAddCondition}
          onExpandedChange={onExpandedChange}
        />
      ))}
    </div>
  );
}

interface SchemaTreeNodeProps {
  node: SchemaNode;
  ancestorPath: string[];
  depth: number;
  selectionState: SelectionState;
  effectiveTargetKey: string | null;
  focusedPathKey: string | null;
  expandedPaths: Set<string>;
  onToggleSelection: (pathKey: string, nodeName: string) => void;
  onSetTarget: (pathKey: string, nodeName: string) => void;
  onAddCondition: (pathKey: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
}

/** Format values for display (like CLI schema output) */
function formatValues(values: string[]): string {
  if (values.length === 0) return '';

  // Detect structural pairs like {}, (), [], <>
  const isStructuralPair = values.length === 2
    && ['{', '(', '[', '<'].includes(values[0])
    && ['}', ')', ']', '>'].includes(values[1]);

  if (isStructuralPair) {
    return `${values[0]}\u2026${values[1]}`;
  }

  if (values.length <= 5) {
    return values.join(', ');
  }

  return `${values.slice(0, 5).join(', ')}, \u2026 (+${values.length - 5})`;
}

function SchemaTreeNode({
  node,
  ancestorPath,
  depth,
  selectionState,
  effectiveTargetKey,
  focusedPathKey,
  expandedPaths,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
}: SchemaTreeNodeProps) {
  const [showMenu, setShowMenu] = useState(false);
  const [showValueMenu, setShowValueMenu] = useState(false);
  const [valueMenuStep, setValueMenuStep] = useState<'operator' | 'value' | null>(null);
  const [selectedOperator, setSelectedOperator] = useState<'exact' | 'contains' | 'starts' | 'ends' | null>(null);
  const nodeRef = useRef<HTMLDivElement>(null);

  // Compute path key for this schema node
  const nodePath = [...ancestorPath, node.name];
  const pathKey = nodePath.join('/');

  const nodeState = selectionState.get(pathKey);
  const isSelected = nodeState?.selected ?? false;
  const isExplicitTarget = nodeState?.isTarget ?? false;
  const isEffectiveTarget = pathKey === effectiveTargetKey;
  const hasCondition = !!nodeState?.condition;
  const isFocused = focusedPathKey === pathKey;

  // Default expansion: expand shallow nodes (depth < 3) or explicitly expanded
  const isExpanded = expandedPaths.has(pathKey) || depth < 3;
  const hasChildren = node.children.length > 0;
  const hasValues = node.values.length > 0;

  // Scroll focused node into view
  useEffect(() => {
    if (isFocused && nodeRef.current) {
      nodeRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }, [isFocused]);

  const handlePillClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleSelection(pathKey, node.name);
  }, [pathKey, node.name, onToggleSelection]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setShowMenu(prev => !prev);
  }, []);

  const handleSetTarget = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onSetTarget(pathKey, node.name);
    setShowMenu(false);
  }, [pathKey, node.name, onSetTarget]);

  const handleAddCondition = useCallback((type: 'exact' | 'contains' | 'starts' | 'ends', value: string) => {
    const escapedText = value.replace(/'/g, "''");
    let condition: string;

    switch (type) {
      case 'exact':
        condition = `.='${escapedText}'`;
        break;
      case 'contains':
        condition = `contains(.,'${escapedText}')`;
        break;
      case 'starts':
        condition = `starts-with(.,'${escapedText}')`;
        break;
      case 'ends':
        condition = `ends-with(.,'${escapedText}')`;
        break;
    }

    // Select the node if not already selected
    if (!isSelected) {
      onToggleSelection(pathKey, node.name);
    }
    onAddCondition(pathKey, condition);
    setShowValueMenu(false);
    setValueMenuStep(null);
    setSelectedOperator(null);
  }, [isSelected, pathKey, node.name, onToggleSelection, onAddCondition]);

  const toggleExpand = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onExpandedChange(
      isExpanded
        ? new Set([...expandedPaths].filter(p => p !== pathKey))
        : new Set([...expandedPaths, pathKey])
    );
  }, [isExpanded, expandedPaths, pathKey, onExpandedChange]);

  const handleBlur = useCallback(() => {
    setTimeout(() => setShowMenu(false), 150);
  }, []);

  const handleValueClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (showValueMenu) {
      setShowValueMenu(false);
      setValueMenuStep(null);
      setSelectedOperator(null);
    } else {
      setShowValueMenu(true);
      setValueMenuStep('operator');
      setShowMenu(false);
    }
  }, [showValueMenu]);

  const handleValueMenuBlur = useCallback(() => {
    setTimeout(() => {
      setShowValueMenu(false);
      setValueMenuStep(null);
      setSelectedOperator(null);
    }, 150);
  }, []);

  const handleSelectOperator = useCallback((op: 'exact' | 'contains' | 'starts' | 'ends') => {
    // For single-value nodes, apply immediately
    if (node.values.length === 1) {
      handleAddCondition(op, node.values[0]);
      return;
    }
    setSelectedOperator(op);
    setValueMenuStep('value');
  }, [node.values, handleAddCondition]);

  // Determine pill classes
  const pillClasses = [
    'node-pill',
    isSelected && 'selected',
    isEffectiveTarget && 'target',
    isEffectiveTarget && !isExplicitTarget && 'auto-target',
    hasCondition && 'has-condition',
    isFocused && 'focused',
    showMenu && 'active',
  ].filter(Boolean).join(' ');

  return (
    <div
      ref={nodeRef}
      className={`tree-node ${isFocused ? 'focused' : ''}`}
      style={{ marginLeft: depth > 0 ? 16 : 0 }}
    >
      <div className="node-row">
        {hasChildren && (
          <button className="expand-btn" onClick={toggleExpand}>
            {isExpanded ? '▼' : '▶'}
          </button>
        )}
        {!hasChildren && <span className="expand-placeholder" />}

        <button
          className={pillClasses}
          onClick={handlePillClick}
          onContextMenu={handleContextMenu}
          onBlur={handleBlur}
          title="Click to select, right-click for options"
        >
          {isEffectiveTarget && <span className="target-marker" title={isExplicitTarget ? 'Explicit target' : 'Auto-detected target (LCA)'}>{isExplicitTarget ? '▶' : '▷'}</span>}
          {node.name}
          {node.count > 1 && <span className="schema-count">({node.count})</span>}
          {hasCondition && <span className="condition-marker">*</span>}
        </button>

        {hasValues && (
          <span
            className="schema-values-wrapper"
            tabIndex={0}
            onBlur={handleValueMenuBlur}
          >
            <span
              className="schema-values"
              title={`${node.values.length} unique value${node.values.length !== 1 ? 's' : ''} — click to add condition`}
              onClick={handleValueClick}
            >
              {formatValues(node.values)}
            </span>

            {showValueMenu && valueMenuStep === 'operator' && (
              <div className="node-menu text-menu">
                <div className="menu-label">{node.values.length} value{node.values.length !== 1 ? 's' : ''}</div>
                <button onMouseDown={() => handleSelectOperator('exact')}>Exactly</button>
                <button onMouseDown={() => handleSelectOperator('contains')}>Contains</button>
                <button onMouseDown={() => handleSelectOperator('starts')}>Starts with</button>
                <button onMouseDown={() => handleSelectOperator('ends')}>Ends with</button>
              </div>
            )}

            {showValueMenu && valueMenuStep === 'value' && selectedOperator && (
              <div className="node-menu text-menu value-list">
                <div className="menu-label">{selectedOperator === 'exact' ? 'Pick value' : `${selectedOperator} ...`}</div>
                {node.values.slice(0, 10).map((value) => (
                  <button
                    key={value}
                    onMouseDown={() => handleAddCondition(selectedOperator, value)}
                    title={value}
                  >
                    {value.length > 25 ? value.slice(0, 25) + '\u2026' : value}
                  </button>
                ))}
                {node.values.length > 10 && (
                  <div className="menu-label">{'\u2026'} +{node.values.length - 10} more</div>
                )}
              </div>
            )}
          </span>
        )}

        {showMenu && (
          <div className="node-menu">
            <button onMouseDown={handleSetTarget}>
              {isExplicitTarget ? 'Unset as target' : 'Set as target'}
            </button>
            <button onMouseDown={handlePillClick}>
              {isSelected ? 'Deselect' : 'Select'}
            </button>
          </div>
        )}
      </div>

      {isExpanded && hasChildren && (
        <div className="node-children">
          {node.children.map((child) => (
            <SchemaTreeNode
              key={child.name}
              node={child}
              ancestorPath={nodePath}
              depth={depth + 1}
              selectionState={selectionState}
              effectiveTargetKey={effectiveTargetKey}
              focusedPathKey={focusedPathKey}
              expandedPaths={expandedPaths}
              onToggleSelection={onToggleSelection}
              onSetTarget={onSetTarget}
              onAddCondition={onAddCondition}
              onExpandedChange={onExpandedChange}
            />
          ))}
        </div>
      )}
    </div>
  );
}
