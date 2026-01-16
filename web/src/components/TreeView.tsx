import { useState, useCallback } from 'react';
import { XmlNode, getXmlNodeId } from '../xmlTree';
import { SelectionState } from '../queryState';

interface TreeViewProps {
  xmlTree: XmlNode | null;
  selectionState: SelectionState;
  onToggleSelection: (nodeId: string, nodeName: string) => void;
  onSetTarget: (nodeId: string, nodeName: string) => void;
  onAddCondition: (nodeId: string, condition: string) => void;
}

export function TreeView({
  xmlTree,
  selectionState,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
}: TreeViewProps) {
  if (!xmlTree) {
    return <div className="tree-view empty">No tree to display</div>;
  }

  return (
    <div className="tree-view">
      <TreeNode
        node={xmlTree}
        selectionState={selectionState}
        onToggleSelection={onToggleSelection}
        onSetTarget={onSetTarget}
        onAddCondition={onAddCondition}
      />
    </div>
  );
}

interface TreeNodeProps {
  node: XmlNode;
  selectionState: SelectionState;
  onToggleSelection: (nodeId: string, nodeName: string) => void;
  onSetTarget: (nodeId: string, nodeName: string) => void;
  onAddCondition: (nodeId: string, condition: string) => void;
}

function TreeNode({
  node,
  selectionState,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
}: TreeNodeProps) {
  const [expanded, setExpanded] = useState(node.depth < 3);
  const [showMenu, setShowMenu] = useState(false);

  const nodeId = getXmlNodeId(node);
  const nodeState = selectionState.get(nodeId);
  const isSelected = nodeState?.selected ?? false;
  const isTarget = nodeState?.isTarget ?? false;
  const hasCondition = !!nodeState?.condition;

  const hasChildren = node.children.length > 0;

  const handlePillClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleSelection(nodeId, node.name);
  }, [nodeId, node.name, onToggleSelection]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setShowMenu(prev => !prev);
  }, []);

  const handleSetTarget = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onSetTarget(nodeId, node.name);
    setShowMenu(false);
  }, [nodeId, node.name, onSetTarget]);

  const handleAddTextCondition = useCallback((type: 'exact' | 'contains' | 'starts' | 'ends') => {
    if (!node.textContent) return;

    const escapedText = node.textContent.replace(/'/g, "''");
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

    onAddCondition(nodeId, condition);
    setShowMenu(false);
  }, [node.textContent, nodeId, onAddCondition]);

  const toggleExpand = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setExpanded(prev => !prev);
  }, []);

  const handleBlur = useCallback(() => {
    setTimeout(() => setShowMenu(false), 150);
  }, []);

  // Determine pill classes based on state
  const pillClasses = [
    'node-pill',
    isSelected && 'selected',
    isTarget && 'target',
    hasCondition && 'has-condition',
    showMenu && 'active',
  ].filter(Boolean).join(' ');

  return (
    <div className="tree-node" style={{ marginLeft: node.depth > 0 ? 16 : 0 }}>
      <div className="node-row">
        {hasChildren && (
          <button className="expand-btn" onClick={toggleExpand}>
            {expanded ? '▼' : '▶'}
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
          {isTarget && <span className="target-marker">→</span>}
          {node.name}
          {hasCondition && <span className="condition-marker">*</span>}
        </button>

        {node.textContent && (
          <span className="node-text" title={node.textContent}>
            {node.textContent.length > 30 ? node.textContent.slice(0, 30) + '...' : node.textContent}
          </span>
        )}

        {showMenu && (
          <div className="node-menu">
            <button onClick={handleSetTarget}>
              {isTarget ? '✓ ' : ''}Set as target
            </button>
            <button onClick={handlePillClick}>
              {isSelected ? 'Deselect' : 'Select'}
            </button>
            {node.textContent && (
              <>
                <hr />
                <div className="menu-label">Match text:</div>
                <button onClick={() => handleAddTextCondition('exact')}>
                  Exactly "{node.textContent.slice(0, 20)}{node.textContent.length > 20 ? '...' : ''}"
                </button>
                <button onClick={() => handleAddTextCondition('contains')}>
                  Contains
                </button>
                <button onClick={() => handleAddTextCondition('starts')}>
                  Starts with
                </button>
                <button onClick={() => handleAddTextCondition('ends')}>
                  Ends with
                </button>
              </>
            )}
          </div>
        )}
      </div>

      {expanded && hasChildren && (
        <div className="node-children">
          {node.children.map((child) => (
            <TreeNode
              key={child.id}
              node={child}
              selectionState={selectionState}
              onToggleSelection={onToggleSelection}
              onSetTarget={onSetTarget}
              onAddCondition={onAddCondition}
            />
          ))}
        </div>
      )}
    </div>
  );
}
