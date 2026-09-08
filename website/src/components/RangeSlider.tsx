import { useEffect, useState } from 'react';
import { clampRange, money } from '../lib/catalog';

interface Props {
  min: number;
  max: number;
  valueMin: number;
  valueMax: number;
  onChange: (min: number, max: number) => void;
}
export function RangeSlider({ min, max, valueMin, valueMax, onChange }: Props) {
  const [low, setLow] = useState(String(valueMin));
  const [high, setHigh] = useState(String(valueMax));
  const [error, setError] = useState('');
  useEffect(() => {
    setLow(String(valueMin));
    setHigh(String(valueMax));
    setError('');
  }, [valueMin, valueMax]);
  const [left, right] = clampRange(Number(low), Number(high), min, max);
  const span = max - min || 1;
  const commit = (lower = Number(low), upper = Number(high)) => {
    if (!low || !high || lower > upper || lower < min || upper > max) {
      setError(`Enter a range between ${money(min)} and ${money(max)} DT.`);
      return;
    }
    setError('');
    onChange(...clampRange(lower, upper, min, max));
  };
  return (
    <form
      className="price-filter"
      onSubmit={(event) => {
        event.preventDefault();
        commit();
      }}
    >
      <div className="range-track">
        <div
          className="range-fill"
          style={{
            left: `${((left - min) / span) * 100}%`,
            right: `${100 - ((right - min) / span) * 100}%`,
          }}
        />
        <input
          aria-label="Minimum price slider"
          type="range"
          min={min}
          max={max}
          value={left}
          disabled={max <= min}
          onChange={(event) => setLow(String(Math.min(Number(event.target.value), right)))}
          onPointerUp={(event) => commit(Math.min(Number(event.currentTarget.value), right), right)}
          onKeyUp={(event) => commit(Math.min(Number(event.currentTarget.value), right), right)}
        />
        <input
          aria-label="Maximum price slider"
          type="range"
          min={min}
          max={max}
          value={right}
          disabled={max <= min}
          onChange={(event) => setHigh(String(Math.max(Number(event.target.value), left)))}
          onPointerUp={(event) => commit(left, Math.max(Number(event.currentTarget.value), left))}
          onKeyUp={(event) => commit(left, Math.max(Number(event.currentTarget.value), left))}
        />
      </div>
      <div className="price-inputs">
        <label>
          Min{' '}
          <span>
            <input
              aria-label="Minimum price"
              type="number"
              min={min}
              max={max}
              step="1"
              value={low}
              onChange={(event) => setLow(event.target.value)}
              onBlur={() => commit()}
            />
            <small>DT</small>
          </span>
        </label>
        <span className="range-separator">—</span>
        <label>
          Max{' '}
          <span>
            <input
              aria-label="Maximum price"
              type="number"
              min={min}
              max={max}
              step="1"
              value={high}
              onChange={(event) => setHigh(event.target.value)}
              onBlur={() => commit()}
            />
            <small>DT</small>
          </span>
        </label>
      </div>
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
