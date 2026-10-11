#!/usr/bin/env python3
"""Run isolated scanner concurrency trials on Linux; never initializes the database.

Build with cargo build --release --bin scan-benchmark, then:
python3 tools/benchmark_scan.py --binary target/release/scan-benchmark --out /home/hazem/retailer-test/scan-benchmark
Pause the production scanner for comparable measurements, keeping its API running.
Requires the same Chromium, WARP and persistent-browser services as production.
"""
import argparse
import json
import os
from pathlib import Path
import random
import signal
import subprocess
import threading
import time
import urllib.request


def read_number(path):
    try:
        return int(Path(path).read_text().strip())
    except (OSError, ValueError):
        return 0


def process_memory(root):
    processes = {}
    for name in os.listdir('/proc'):
        if not name.isdigit():
            continue
        try:
            values = dict(line.split(':', 1) for line in Path('/proc', name, 'status').read_text().splitlines() if ':' in line)
            processes[int(name)] = (int(values['PPid']), int(values.get('VmRSS', '0 kB').split()[0]))
        except (OSError, ValueError, KeyError):
            continue
    children = {root}
    while True:
        expanded = children | {pid for pid, (parent, _) in processes.items() if parent in children}
        if expanded == children:
            break
        children = expanded
    return sum(processes.get(pid, (0, 0))[1] for pid in children) * 1024


def run(binary, arguments, log, timeout=600):
    measurements = {'browser_peak_bytes': 0, 'browser_tab_peak': 0, 'benchmark_tree_peak_bytes': 0,
                    'available_min_bytes': None, 'memory_stop': False}
    finished = threading.Event()

    def sample(pid):
        while not finished.is_set():
            browser = read_number('/sys/fs/cgroup/system.slice/flagship-browser.service/memory.current')
            measurements['browser_peak_bytes'] = max(measurements['browser_peak_bytes'], browser)
            measurements['benchmark_tree_peak_bytes'] = max(measurements['benchmark_tree_peak_bytes'], process_memory(pid))
            try:
                with urllib.request.urlopen('http://127.0.0.1:9224/json/list', timeout=1) as response:
                    pages = [page for page in json.load(response) if page.get('type') == 'page']
                measurements['browser_tab_peak'] = max(measurements['browser_tab_peak'], len(pages))
            except (OSError, ValueError):
                pass
            try:
                available = next(int(line.split()[1])*1024 for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:'))
                previous = measurements['available_min_bytes']
                measurements['available_min_bytes'] = available if previous is None else min(previous, available)
                if available < 512*1024**2 and not measurements['memory_stop']:
                    measurements['memory_stop'] = True
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
            except (OSError, StopIteration):
                pass
            finished.wait(0.5)

    with log.open('w') as output:
        process = subprocess.Popen([str(binary), *arguments], stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        sampler = threading.Thread(target=sample, args=(process.pid,), daemon=True)
        sampler.start()
        try:
            code = process.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code = 124
            if isinstance(error, KeyboardInterrupt):
                raise
        finally:
            finished.set()
            sampler.join()
    return code, measurements


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/scan-benchmark'))
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--fixture', type=Path)
    parser.add_argument('--samples', type=int, default=8)
    parser.add_argument('--repeats', type=int, default=2)
    parser.add_argument('--cold-rounds', type=int, default=1, help='repeat empty-cache details in each process to exercise tab recycling')
    parser.add_argument('--retailers', default='Mytek,MediaVision,CyberInfo,Lazari,Batam,SkyMil-Shop,Agora,Tunisianet,SpaceNet,SigShop,MBMInformatique,ExpertGaming')
    parser.add_argument('--configs', default='1:1:4:2,2:1:8:4,2:2:20:4,4:2:20:4,4:4:20:4,6:2:20:4,8:4:24:4',
                        help='browser:browser-per-host:http:http-per-host, comma separated')
    args = parser.parse_args()
    if args.repeats < 1 or args.samples < 1 or args.cold_rounds < 1:
        parser.error('repeats, samples and cold-rounds must be positive')
    binary = args.binary.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    fixture = args.fixture or args.out / 'fixture.json'
    if not args.fixture:
        print('Preparing fixed retailer workload...', flush=True)
        code, _ = run(binary, ['prepare', str(fixture), '--samples', str(args.samples), '--retailers', args.retailers], args.out/'prepare.log')
        if code:
            raise SystemExit(f'Fixture preparation failed; inspect {args.out / "prepare.log"}')
    configurations = [tuple(int(value) for value in config.split(':')) for config in args.configs.split(',')]
    if any(len(config) != 4 or min(config) < 1 for config in configurations):
        parser.error('each configuration needs four positive integers')
    trials = [(config, repeat) for repeat in range(1, args.repeats+1) for config in configurations]
    random.Random(20261011).shuffle(trials)
    results = []
    for (browser, host, http, http_host), repeat in trials:
        name = f'b{browser}-h{host}-http{http}-hh{http_host}-r{repeat}'
        output = args.out / f'{name}.json'
        print(f'Starting {name}', flush=True)
        code, memory = run(binary, ['run', str(fixture), '--browser', str(browser), '--per-host', str(host),
                                 '--http', str(http), '--http-per-host', str(http_host), '--cold-rounds', str(args.cold_rounds),
                                 '--output', str(output)], args.out/f'{name}.log')
        result = json.loads(output.read_text()) if output.exists() else {'valid': False, 'limits': {'browser':browser, 'per_host':host, 'http':http, 'http_per_host':http_host}}
        result.update({'trial':name, 'exit_code':code, 'memory':memory})
        output.write_text(json.dumps(result, indent=2))
        results.append(result)
        print(f'{name}: valid={result["valid"]} cold={result.get("cold_seconds", 0):.2f}s warm={result.get("warm_seconds", 0):.4f}s browser_peak={memory["browser_peak_bytes"]/1024**2:.0f}MiB', flush=True)
        (args.out/'results.json').write_text(json.dumps(results, indent=2))
        if memory['available_min_bytes'] is not None and memory['available_min_bytes'] < 512*1024**2:
            raise SystemExit('Stopped benchmark: less than 512 MiB available memory; results saved.')
    summary = []
    import statistics
    for config in configurations:
        runs = [result for result in results if tuple(result['limits'][key] for key in ('browser', 'per_host', 'http', 'http_per_host')) == config]
        summary.append({'configuration':config, 'valid':all(result['valid'] and result['exit_code'] == 0 for result in runs),
                        'median_cold_seconds':statistics.median(result.get('cold_seconds', float('inf')) for result in runs),
                        'median_category_seconds':statistics.median(result.get('category_seconds', float('inf')) for result in runs),
                        'browser_peak_mib':max(result['memory']['browser_peak_bytes']/1024**2 for result in runs)})
    summary.sort(key=lambda item: (not item['valid'], item['median_cold_seconds'] + item['median_category_seconds']))
    (args.out/'summary.json').write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary, indent=2), flush=True)
    if not any(item['valid'] for item in summary):
        raise SystemExit('No configuration passed validation; inspect trial logs.')


if __name__ == '__main__':
    main()
