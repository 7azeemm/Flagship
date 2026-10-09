import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { Cpu, Zap } from 'lucide-react';
import type { ComponentDetails } from '../types';
import { parseComponentDetails, request } from '../lib/api';
import { componentSpecificationRows } from '../lib/componentDetails';
import { Dialog } from './Dialog';

export function ComponentDetailsDialog({
  component,
  onClose,
}: {
  component: Pick<ComponentDetails, 'kind' | 'id' | 'label'> & { platform: 'desktop' | 'laptop' };
  onClose: () => void;
}) {
  const [details, setDetails] = useState<ComponentDetails | null>(null);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    setDetails(null);
    setError('');
    const query = new URLSearchParams({ id: component.id });
    if (component.kind === 'GPU') query.set('platform', component.platform);
    void request(`/components/${component.kind.toLowerCase()}?${query}`, controller.signal)
      .then(parseComponentDetails)
      .then((result) => {
        if (!controller.signal.aborted) setDetails(result);
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(
            cause instanceof Error ? cause.message : 'Component details could not be loaded.',
          );
      });
    return () => controller.abort();
  }, [component.kind, component.id, component.platform, revision]);
  const Icon = component.kind === 'CPU' ? Cpu : Zap;
  const rows = details ? componentSpecificationRows(details) : [];
  return createPortal(
    <Dialog
      title={`${component.kind} details: ${details?.label || component.label}`}
      onClose={onClose}
      className="component-details-dialog"
    >
      <div className="component-details-heading">
        <Icon size={24} />
        <div>
          <span className="eyebrow">{component.kind} specifications</span>
          <h2>{details?.label || component.label}</h2>
        </div>
      </div>
      {!details ? (
        <div role={error ? 'alert' : 'status'}>
          <p className="muted">{error || 'Loading component details…'}</p>
          {error && (
            <button
              type="button"
              className="primary-button"
              onClick={() => setRevision((value) => value + 1)}
            >
              Try again
            </button>
          )}
        </div>
      ) : rows.length ? (
        <dl className="component-details-facts">
          {rows.map(({ key, label, value }) => (
            <div key={key}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className="muted">Detailed specifications are not available for this component yet.</p>
      )}
    </Dialog>,
    document.body,
  );
}
