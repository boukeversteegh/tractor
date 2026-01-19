import { useState, useCallback, useRef, useEffect } from 'react';
import { XmlNode, getXmlNodeId, getNodeNamePath, pathToKey } from '../xmlTree';
import { SelectionState } from '../queryState';

interface TreeViewProps {
  xmlTree: XmlNode | null;
  selectionState: SelectionState;
  effectiveTargetKey: string | null;  // Path key of effective target
  focusedNodeId: string | null;
  expandedNodeIds: Set<string>;
  onToggleSelection: (pathKey: string, nodeName: string) => void;
  onSetTarget: (pathKey: string, nodeName: string) => void;
  onAddCondition: (pathKey: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
  onNodeHover?: (node: XmlNode | null) => void;
}

export function TreeView({
  xmlTree,
  selectionState,
  effectiveTargetKey,
  focusedNodeId,
  expandedNodeIds,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
  onNodeHover,
}: TreeViewProps) {
  if (!xmlTree) {
    return <div className="tree-view empty">No tree to display</div>;
  }

  return (
    <div className="tree-view">
      <TreeNode
        node={xmlTree}
        ancestorPath={[]}
        selectionState={selectionState}
        effectiveTargetKey={effectiveTargetKey}
        focusedNodeId={focusedNodeId}
        expandedNodeIds={expandedNodeIds}
        onToggleSelection={onToggleSelection}
        onSetTarget={onSetTarget}
        onAddCondition={onAddCondition}
        onExpandedChange={onExpandedChange}
        onNodeHover={onNodeHover}
      />
    </div>
  );
}

interface TreeNodeProps {
  node: XmlNode;
  ancestorPath: string[];  // Path of ancestor node names (not including this node)
  selectionState: SelectionState;
  effectiveTargetKey: string | null;
  focusedNodeId: string | null;
  expandedNodeIds: Set<string>;
  onToggleSelection: (pathKey: string, nodeName: string) => void;
  onSetTarget: (pathKey: string, nodeName: string) => void;
  onAddCondition: (pathKey: string, condition: string) => void;
  onExpandedChange: (expanded: Set<string>) => void;
  onNodeHover?: (node: XmlNode | null) => void;
}

function TreeNode({
  node,
  ancestorPath,
  selectionState,
  effectiveTargetKey,
  focusedNodeId,
  expandedNodeIds,
  onToggleSelection,
  onSetTarget,
  onAddCondition,
  onExpandedChange,
  onNodeHover,
}: TreeNodeProps) {
  const [showMenu, setShowMenu] = useState(false);
  const [showTextMenu, setShowTextMenu] = useState(false);
  const [textSelection, setTextSelection] = useState<{
    text: string;
    isStart: boolean;
    isEnd: boolean;
    isAll: boolean;
  } | null>(null);
  const nodeRef = useRef<HTMLDivElement>(null);
  const textMenuRef = useRef<HTMLSpanElement>(null);

  // Compute the path key for this node (used for selection state)
  const nodePath = getNodeNamePath(node, ancestorPath);
  const pathKey = pathToKey(nodePath);

  // Node ID is still used for expansion/focus (unique per instance)
  const nodeId = getXmlNodeId(node);
  const nodeState = selectionState.get(pathKey);
  const isSelected = nodeState?.selected ?? false;
  const isExplicitTarget = nodeState?.isTarget ?? false;
  const isEffectiveTarget = pathKey === effectiveTargetKey;
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

  const handleAddTextCondition = useCallback((type: 'exact' | 'contains' | 'starts' | 'ends') => {
    const matchText = textSelection?.text || node.textContent;
    if (!matchText) return;

    const escapedText = matchText.replace(/'/g, "''");
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

    onAddCondition(pathKey, condition);
    setShowMenu(false);
    setShowTextMenu(false);
    setTextSelection(null);
  }, [textSelection, node.textContent, pathKey, onAddCondition]);

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

  const handleTextMenuBlur = useCallback(() => {
    setTimeout(() => {
      setShowTextMenu(false);
      setTextSelection(null);
    }, 150);
  }, []);

  const handleTextMouseUp = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();

    const fullText = node.textContent || '';
    const selection = window.getSelection();
    const selectedText = selection?.toString() || '';

    // If menu is open and no new selection, close it
    if (showTextMenu && !selectedText) {
      setShowTextMenu(false);
      setTextSelection(null);
      return;
    }

    // Determine selection position relative to full text
    let isStart = false;
    let isEnd = false;
    let isAll = false;
    let matchText = selectedText;

    if (selectedText && selectedText.length > 0) {
      const startIndex = fullText.indexOf(selectedText);
      if (startIndex !== -1) {
        isStart = startIndex === 0;
        isEnd = startIndex + selectedText.length === fullText.length;
        isAll = isStart && isEnd;
      }
      matchText = selectedText;
    } else {
      // No selection - use full text
      matchText = fullText;
      isAll = true;
      isStart = true;
      isEnd = true;
    }

    // Select the node if not already selected
    if (!isSelected) {
      onToggleSelection(pathKey, node.name);
    }

    setTextSelection({ text: matchText, isStart, isEnd, isAll });
    setShowTextMenu(true);
    setShowMenu(false);
  }, [node.textContent, isSelected, pathKey, node.name, onToggleSelection, showTextMenu]);

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
          onMouseEnter={() => onNodeHover?.(node)}
          onMouseLeave={() => onNodeHover?.(null)}
          title="Click to select, right-click for options"
        >
          {isEffectiveTarget && <span className="target-marker" title={isExplicitTarget ? 'Explicit target' : 'Auto-detected target (LCA)'}>{isExplicitTarget ? '▶' : '▷'}</span>}
          {node.name}
          {hasCondition && <span className="condition-marker">*</span>}
        </button>

        {node.textContent && (
          <span
            className="node-text selectable"
            title="Select text or click to add condition"
            onMouseUp={handleTextMouseUp}
            ref={textMenuRef}
            tabIndex={0}
            onBlur={handleTextMenuBlur}
          >
            {node.textContent}
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
          </div>
        )}

        {showTextMenu && textSelection && (
          <div className="node-menu text-menu">
            <div className="menu-label">"{textSelection.text.slice(0, 20)}{textSelection.text.length > 20 ? '...' : ''}"</div>
            {textSelection.isAll && (
              <button onClick={() => handleAddTextCondition('exact')}>
                Exactly
              </button>
            )}
            {!textSelection.isAll && (
              <button onClick={() => handleAddTextCondition('contains')}>
                Contains
              </button>
            )}
            {textSelection.isStart && !textSelection.isAll && (
              <button onClick={() => handleAddTextCondition('starts')}>
                Starts with
              </button>
            )}
            {textSelection.isEnd && !textSelection.isAll && (
              <button onClick={() => handleAddTextCondition('ends')}>
                Ends with
              </button>
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
              ancestorPath={nodePath}
              selectionState={selectionState}
              effectiveTargetKey={effectiveTargetKey}
              focusedNodeId={focusedNodeId}
              expandedNodeIds={expandedNodeIds}
              onToggleSelection={onToggleSelection}
              onSetTarget={onSetTarget}
              onAddCondition={onAddCondition}
              onExpandedChange={onExpandedChange}
              onNodeHover={onNodeHover}
            />
          ))}
        </div>
      )}
    </div>
  );
}
