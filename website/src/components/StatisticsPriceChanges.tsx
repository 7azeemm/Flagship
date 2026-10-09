import { ArrowDownRight, ArrowUpRight, BarChart3 } from 'lucide-react';
import type { StatisticsData } from '../types';
import { money } from '../lib/catalog';
import { shortDate } from '../lib/insights';
import '../price-change-summary.css';

const directions = [
  { id: 'fell', label: 'Prices fell', icon: ArrowDownRight, movement: 'drop' },
  { id: 'rose', label: 'Prices rose', icon: ArrowUpRight, movement: 'increase' },
] as const;

export function StatisticsPriceChanges({ data }: { data: StatisticsData['price_changes'] }) {
  const changed = (data?.fell ?? 0) + (data?.rose ?? 0);
  const bands = data?.magnitude_bands;
  const maximum = Math.max(1, ...(bands?.flatMap((band) => [band.fell, band.rose]) ?? []));
  return (
    <article className="statistics-card price-changes-card">
      <div className="statistics-card-heading">
        <div>
          <h3>Price changes</h3>
          <p>See the size of recorded price drops and increases · all stock statuses.</p>
        </div>
      </div>
      {!data?.compared_offers ? (
        <div className="statistics-empty">
          <BarChart3 size={26} />
          <strong>No comparable price history yet</strong>
          <span>Offers need a positive recorded price at both dates.</span>
        </div>
      ) : !changed ? (
        <div className="statistics-empty">
          <BarChart3 size={26} />
          <strong>No price changes in this period</strong>
          <span>Try a longer period to see earlier movements.</span>
        </div>
      ) : (
        <div className="price-change-layout">
          <div className="price-change-totals" aria-label="Prices that changed">
            {directions.map(({ id, label, icon: Icon, movement }) => {
              const average = data.summary?.[id].average_percent;
              return (
                <div className={`price-change-total change-${id}`} key={id}>
                  <span>
                    <Icon size={16} aria-hidden="true" />
                    {label}
                  </span>
                  <strong>
                    {money(data[id])}
                    <small>{data[id] === 1 ? 'offer' : 'offers'}</small>
                  </strong>
                  <p>
                    {average != null
                      ? `${Math.abs(average).toFixed(1)}% average ${movement}`
                      : data[id]
                        ? 'Recorded during this period'
                        : `No ${movement}s recorded`}
                  </p>
                </div>
              );
            })}
          </div>
          {bands ? (
            <figure className="price-change-size">
              <figcaption>
                <h4>Size of the change</h4>
                <div className="price-change-key" aria-hidden="true">
                  <span>
                    <i className="is-fall" />
                    Fell
                  </span>
                  <span>
                    <i className="is-rise" />
                    Rose
                  </span>
                </div>
              </figcaption>
              <div
                className="price-change-columns"
                role="img"
                aria-label={bands
                  .map(
                    (band) =>
                      `${band.to === null ? `${band.from}% or more` : `${band.from}% to under ${band.to}%`}: ${band.fell} prices fell, ${band.rose} prices rose`,
                  )
                  .join('; ')}
              >
                {bands.map((band) => (
                  <div className="price-change-column-group" key={band.from}>
                    <div className="price-change-column-pair">
                      {directions.map(({ id, label }) => (
                        <div
                          className={`price-change-column change-${id}`}
                          key={id}
                          title={`${label}: ${money(band[id])} offers`}
                        >
                          <strong>{money(band[id])}</strong>
                          <span style={{ height: `${(84 * band[id]) / maximum}%` }} />
                        </div>
                      ))}
                    </div>
                    <span className="price-change-band-label">
                      {band.to === null
                        ? `${band.from}%+`
                        : band.from === 0
                          ? 'Under 2%'
                          : `${band.from}–${band.to}%`}
                    </span>
                  </div>
                ))}
              </div>
              <p>Number of offers · only prices that changed</p>
            </figure>
          ) : (
            <div className="statistics-empty">
              <BarChart3 size={24} />
              <strong>Change sizes are waiting for the updated backend.</strong>
            </div>
          )}
        </div>
      )}
      {data && (
        <div className="statistics-card-meta">
          <span>
            {data.since
              ? `${shortDate(data.since)} – ${shortDate(data.until)}`
              : 'No earlier price snapshot'}
          </span>
          <span>{money(data.compared_offers)} comparable offers</span>
        </div>
      )}
      <details>
        <summary>How this is calculated</summary>
        <p>
          Each listing’s positive price is compared at the start and end of available history in
          your period. The chart groups the percentage size of its net change; upper bounds belong
          to the next band. Offers without a price change are left out. New offers without a
          starting price and zero or unknown prices cannot be compared. All stock statuses are
          included. Averages give each changed offer equal weight. Retailer original prices are not
          used.
        </p>
      </details>
    </article>
  );
}
