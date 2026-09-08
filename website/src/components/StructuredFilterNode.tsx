import { useEffect, useId, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import type { FilterValue } from '../types';
import { isWithin, nodeStatus } from '../lib/catalog';

interface Props {
  node: FilterValue;
  depth?: number;
  filterKey: string;
  selectedIds: string[];
  onToggle: (key: string, id: string) => void;
}
export function StructuredFilterNode({ node, filterKey, selectedIds, onToggle, depth = 0 }: Props) {
  const [expanded, setExpanded] = useState(() => selectedIds.some((id) => isWithin(id, node.id)));
  const checkbox = useRef<HTMLInputElement>(null);
  const id = useId();
  const status = nodeStatus(node, selectedIds);
  useEffect(() => {
    if (checkbox.current) checkbox.current.indeterminate = status === 'partial';
  }, [status]);
  return (
    <div className="filter-node">
      <div className="filter-node-row">
        <label
          className="checkbox-row"
          title={node.label}
          style={{ paddingLeft: Math.min(depth, 4) * 8 }}
        >
          <input
            ref={checkbox}
            type="checkbox"
            checked={status === 'checked'}
            onChange={() => onToggle(filterKey, node.id)}
          />
          <span>{node.label}</span>
        </label>
        <small className="filter-count">{node.count}</small>
        {!!node.children?.length ? (
          <button
            className="tree-toggle"
            aria-label={`${expanded ? 'Collapse' : 'Expand'} ${node.label}`}
            aria-expanded={expanded}
            aria-controls={id}
            onClick={() => setExpanded(!expanded)}
          >
            <ChevronDown size={15} />
          </button>
        ) : (
          <span className="tree-toggle-spacer" />
        )}
      </div>
      {!!node.children?.length && expanded && (
        <div id={id} className="filter-children">
          {node.children.map((child) => (
            <StructuredFilterNode
              key={child.id}
              depth={depth + 1}
              node={child}
              filterKey={filterKey}
              selectedIds={selectedIds}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
