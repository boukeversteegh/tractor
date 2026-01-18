import { useState, useCallback, useRef, useEffect } from 'react';
import { XmlNode, getXmlNodeId } from '../xmlTree';
import { SelectionState } from '../queryState';

interface TreeViewProps {
  xmlTree: XmlNode | null;
  selectionState: SelectionState;
  effectiveTargetId: string | null;
  focusedNodeId: string | null;
  expandedNodeIds: Set<string>;
  onToggleSelection: (nodeId: string, nodeName: string) => void;
  onSetTarget: (nodeId: string, nodeName: string) => void;
  onAddCondition: (nodeId: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
}

export function TreeView({
  xmlTree,
  selectionState,
  effectiveTargetId,
  focusedNodeId,
  expandedNodeIds,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
}: TreeViewProps) {
  if (!xmlTree) {
    return <div className="tree-view empty">No tree to display</div>;
  }

  return (
    <div className="tree-view">
      <TreeNode
        node={xmlTree}
        selectionState={selectionState}
        effectiveTargetId={effectiveTargetId}
        focusedNodeId={focusedNodeId}
        expandedNodeIds={expandedNodeIds}
        onToggleSelection={onToggleSelection}
        onSetTarget={onSetTarget}
        onAddCondition={onAddCondition}
        onExpandedChange={onExpandedChange}
      />
    </div>
  );
}

interface TreeNodeProps {
  node: XmlNode;
  selectionState: SelectionState;
  effectiveTargetId: string | null;
  focusedNodeId: string | null;
  expandedNodeIds: Set<string>;
  onToggleSelection: (nodeId: string, nodeName: string) => void;
  onSetTarget: (nodeId: string, nodeName: string) => void;
  onAddCondition: (nodeId: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
}

function TreeNode({
  node,
  selectionState,
  effectiveTargetId,
  focusedNodeId,
  expandedNodeIds,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
}: TreeNodeProps) {
  const [showMenu, setShowMenu] = useState(false);
  const nodeRef = useRef<HTMLDivElement>(null);

  const nodeId = getXmlNodeId(node);
  const nodeState = selectionState.get(nodeId);
  const isSelected = nodeState?.selected ?? false;
  const isExplicitTarget = nodeState?.isTarget ?? false;
  const isEffectiveTarget = nodeId === effectiveTargetId;
  const hasCondition = !!nodeState?.condition;
  const isFocused = focusedNodeId === nodeId;

  // Use expandedNodeIds from parent, with default expansion for shallow nodes
  const isExpanded = expandedNodeIds.has(nodeId) || node.depth < 3;
  const hasChildren = node.children.length > 0;

  // Scroll focused node into view
  useEffect(() => {
    if (isFocused && nodeRef.current) {
      nodeRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }, [isFocused]);

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
    onExpandedChange(
      isExpanded
        ? new Set([...expandedNodeIds].filter(id => id !== nodeId))
        : new Set([...expandedNodeIds, nodeId])
    );
  }, [isExpanded, expandedNodeIds, nodeId, onExpandedChange]);

  const handleBlur = useCallback(() => {
    setTimeout(() => setShowMenu(false), 150);
  }, []);

  const handleTextClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    // Select the node if not already selected
    if (!isSelected) {
      onToggleSelection(nodeId, node.name);
    }
    // Open the menu to show text condition options
    setShowMenu(true);
  }, [isSelected, nodeId, node.name, onToggleSelection]);

  // Determine pill classes based on state
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
      style={{ marginLeft: node.depth > 0 ? 16 : 0 }}
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
          {hasCondition && <span className="condition-marker">*</span>}
        </button>

        {node.textContent && (
          <span
            className="node-text clickable"
            title="Click to add text condition"
            onClick={handleTextClick}
          >
            {node.textContent.length > 30 ? node.textContent.slice(0, 30) + '...' : node.textContent}
          </span>
        )}

        {showMenu && (
          <div className="node-menu">
            <button onClick={handleSetTarget}>
              {isExplicitTarget ? 'Unset as target' : 'Set as target'}
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

      {isExpanded && hasChildren && (
        <div className="node-children">
          {node.children.map((child) => (
            <TreeNode
              key={child.id}
              node={child}
              selectionState={selectionState}
              effectiveTargetId={effectiveTargetId}
              focusedNodeId={focusedNodeId}
              expandedNodeIds={expandedNodeIds}
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
