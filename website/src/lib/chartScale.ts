export function zeroBasedAxis(
  values: number[],
  {
    maximumValue = Infinity,
    integerTicks = false,
  }: { maximumValue?: number; integerTicks?: boolean } = {},
) {
  const highest = Math.min(maximumValue, Math.max(0, ...values.filter(Number.isFinite)));
  const rawStep = Math.max(1, highest / 5);
  const magnitude = 10 ** Math.floor(Math.log10(rawStep));
  const niceStep =
    ([1, 2, 2.5, 5, 10].find((unit) => unit * magnitude >= rawStep) || 10) * magnitude;
  const step = integerTicks ? Math.ceil(niceStep) : niceStep;
  const maximum = Math.min(maximumValue, Math.max(step, Math.ceil(highest / step) * step));
  const ticks = Array.from({ length: Math.floor(maximum / step) + 1 }, (_, index) => index * step);
  if (ticks[ticks.length - 1] !== maximum) ticks.push(maximum);
  return { maximum, ticks };
}

export function trendBounds(
  values: number[],
  {
    baseline,
    minimumPadding = 1,
    maximumValue,
  }: { baseline?: number; minimumPadding?: number; maximumValue?: number } = {},
) {
  const lowest = Math.min(...values, baseline ?? Infinity);
  const highest = Math.max(...values, baseline ?? -Infinity);
  const padding = Math.max(minimumPadding, (highest - lowest) * 0.15);
  const low = baseline === undefined ? Math.max(0, lowest - padding) : lowest - padding;
  const high = Math.min(
    maximumValue ?? Infinity,
    Math.max(low + minimumPadding * 2, highest + padding),
  );
  return { low, high };
}
