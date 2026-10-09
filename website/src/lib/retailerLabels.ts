type StorePoint = { name: string; value: number; inStock: number };
export type RetailerLabel = {
  name: string;
  x: number;
  y: number;
  width: number;
  dotX: number;
  dotY: number;
};

export function retailerLabelLayout(
  stores: StorePoint[],
  width: number,
  maximum: number,
  priceMaximum = 100,
) {
  const left = 58;
  const right = width - 24;
  const top = 40;
  const labelHeight = 23;
  const height = Math.max(404, 404 + Math.max(0, stores.length - 18) * 28);
  const bottom = height - 60;
  const points = stores.map((store) => ({
    name: store.name,
    x: left + (store.value / priceMaximum) * (right - left),
    y: bottom - (store.inStock / maximum) * (bottom - top),
    width: Math.min(right - left, store.name.length * 6.4 + 12),
  }));
  const labels: RetailerLabel[] = [];
  const ordered = [...points].sort((a, b) => b.width - a.width || a.y - b.y);
  // Revisit nearby placements to give crowded labels room without moving them away.
  for (let pass = 0; pass < 8; pass++) {
    for (const point of ordered) {
      // Choose only positions beside the dot, even when the chart is crowded.
      const positions = [
        { x: point.x + 10, y: point.y - labelHeight / 2 },
        { x: point.x - point.width - 10, y: point.y - labelHeight / 2 },
        { x: point.x + 8, y: point.y - labelHeight - 6 },
        { x: point.x - point.width - 8, y: point.y - labelHeight - 6 },
        { x: point.x + 8, y: point.y + 6 },
        { x: point.x - point.width - 8, y: point.y + 6 },
        { x: point.x - point.width / 2, y: point.y - labelHeight - 10 },
        { x: point.x - point.width / 2, y: point.y + 10 },
        { x: point.x - 12, y: point.y - labelHeight - 10 },
        { x: point.x - point.width + 12, y: point.y - labelHeight - 10 },
        { x: point.x - 12, y: point.y + 10 },
        { x: point.x - point.width + 12, y: point.y + 10 },
        { x: point.x - point.width / 2, y: point.y - labelHeight - 14 },
        { x: point.x - point.width / 2, y: point.y + 14 },
      ];
      let best: RetailerLabel | undefined;
      let bestOverlap = Infinity;
      for (const position of positions) {
        const x = Math.max(left, Math.min(right - point.width, position.x));
        const y = position.y;
        if (y < top - 12 || y + labelHeight > bottom + 12) continue;
        const gapX = Math.max(x - point.x, point.x - x - point.width, 0);
        const gapY = Math.max(y - point.y, point.y - y - labelHeight, 0);
        if (gapX ** 2 + gapY ** 2 < 64) continue;
        const overlap =
          labels
            .filter((label) => label.name !== point.name)
            .reduce(
              (total, label) =>
                total +
                Math.max(
                  0,
                  Math.min(x + point.width + 3, label.x + label.width + 3) - Math.max(x, label.x),
                ) *
                  Math.max(
                    0,
                    Math.min(y + labelHeight + 3, label.y + labelHeight + 3) - Math.max(y, label.y),
                  ),
              0,
            ) +
          points.reduce(
            (total, other) =>
              total +
              (other.x > x - 7 &&
              other.x < x + point.width + 7 &&
              other.y > y - 7 &&
              other.y < y + labelHeight + 7
                ? 500
                : 0),
            0,
          );
        if (overlap >= bestOverlap) continue;
        bestOverlap = overlap;
        best = { name: point.name, x, y, width: point.width, dotX: point.x, dotY: point.y };
      }
      if (best) {
        const index = labels.findIndex((label) => label.name === point.name);
        if (index === -1) labels.push(best);
        else labels[index] = best;
      }
    }
  }
  return { height, bottom, labels };
}
