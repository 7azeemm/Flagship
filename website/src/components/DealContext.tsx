import type { DealEvidence } from '../types';
import { fieldLabel, money } from '../lib/catalog';

export function DealContext({ evidence: e }: { evidence: DealEvidence }) {
  return (
    <section className="deal-context" aria-label="Flagship deal evidence">
      <span className="eyebrow">Flagship price check</span>
      <div className="deal-context-badges">
        {e.new_low && <strong>New 30-day low</strong>}
        {!e.new_low && e.recent_drop && <strong>Recent price drop</strong>}
        {e.gap_percent !== null && <strong>Below next-best offer</strong>}
      </div>
      {e.gap_percent !== null && e.next_best_price !== null && (
        <p>
          <b>{e.gap_percent.toFixed(1)}% below</b> the next-best offer of {money(e.next_best_price)}{' '}
          DT at {e.next_best_retailer}. Compared across {e.retailer_count} distinct retailers with
          in-stock offers.
        </p>
      )}
      {e.history_saving_percent !== null && (
        <p>
          {e.history_saving_percent.toFixed(1)}% below the typical recorded price of{' '}
          {money(e.typical_price!)} DT.{' '}
          {e.new_low
            ? 'Lowest recorded by Flagship over the last 30 days.'
            : 'Price fell within the last seven days.'}
        </p>
      )}
      {!e.enough_history && (
        <p className="muted">Not enough history for a historical deal rating yet.</p>
      )}
      {e.enough_history && !e.new_low && !e.recent_drop && e.gap_percent === null && (
        <p>No qualifying deal detected at this price.</p>
      )}
      {Object.keys(e.comparison_fields).length > 0 && (
        <details>
          <summary>Compared specifications</summary>
          <dl>
            {Object.entries(e.comparison_fields).map(([key, value]) => (
              <div key={key}>
                <dt>{fieldLabel(key)}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
        </details>
      )}
      <small>
        Based on recorded prices and matching offers. Retailer-advertised discounts are excluded.
      </small>
    </section>
  );
}
