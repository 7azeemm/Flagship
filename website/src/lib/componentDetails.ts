import type { ComponentDetails, ComponentValue } from '../types';

export function componentDetailsTarget(
  key: string,
  value: ComponentValue | undefined,
  section: string,
) {
  if ((key !== 'cpu' && key !== 'gpu') || !value || typeof value === 'string' || !value.id.trim())
    return null;
  return {
    kind: key === 'cpu' ? ('CPU' as const) : ('GPU' as const),
    id: value.id,
    label: value.label,
    platform: ['Laptop', 'GamingLaptop', 'MacBook'].includes(section)
      ? ('laptop' as const)
      : ('desktop' as const),
  };
}

const labels: Record<string, string> = {
  core_count: 'Cores',
  thread_count: 'Threads',
  base_clock: 'Base clock',
  boost_clock: 'Boost clock',
  tdp: 'TDP',
  l1_cache: 'L1 cache',
  l2_cache: 'L2 cache',
  l3_cache: 'L3 cache',
  total_cache: 'Total cache',
  socket: 'Socket',
  memory_support: 'Memory support',
  integrated_gpu: 'Integrated graphics',
  platform: 'Platform',
  memory_size: 'Memory size',
  gpu: 'GPU chipset',
  transistors: 'Transistors',
  bus_interface: 'Bus interface',
  memory_type: 'Memory type',
  memory_bus: 'Memory bus',
  memory_bandwidth: 'Memory bandwidth',
  cores: 'Shader cores',
  tensor_cores: 'Tensor cores',
  rt_cores: 'Ray tracing cores',
  t_flops: 'FP32 performance',
};

export function componentSpecificationRows(details: ComponentDetails) {
  const units: Record<string, string> = {
    base_clock: details.kind === 'CPU' ? 'GHz' : 'MHz',
    boost_clock: details.kind === 'CPU' ? 'GHz' : 'MHz',
    tdp: 'W',
    l1_cache: 'KB',
    l2_cache: 'MB',
    l3_cache: 'MB',
    total_cache: 'MB',
    memory_size: 'GB',
    transistors: 'billion',
    memory_bus: 'bit',
    memory_bandwidth: 'GB/s',
    t_flops: 'TFLOPS',
  };
  return Object.entries(details.specifications)
    .filter(([key, value]) => key !== 'name' && key !== 'vendor_card' && String(value).trim())
    .map(([key, raw]) => {
      let value = typeof raw === 'boolean' ? (raw ? 'Yes' : 'No') : String(raw);
      if (key === 'platform')
        value =
          value === 'both'
            ? 'Desktop and laptop'
            : value === 'laptop'
              ? 'Laptop'
              : value === 'desktop'
                ? 'Desktop'
                : value;
      if (units[key] && /^\d+(\.\d+)?$/.test(value)) value += ` ${units[key]}`;
      return { key, label: labels[key] || key.replaceAll('_', ' '), value };
    });
}
