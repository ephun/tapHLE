#!/usr/bin/env python3
"""Run visible simulator replays; collect evidence, never infer gameplay from logs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def validate_route(route):
    """Check executable input before launching (including common swipe typos)."""
    if route.get('clickmap_version') != 1 or not route.get('steps'):
        raise ValueError('requires a version 1 clickmap with steps')
    defaults = route.get('defaults', {})
    duration = 0
    for step in route['steps']:
        action = step.get('action')
        required = {'tap': ['at'], 'swipe': ['from_xy', 'to_xy'],
                    'wait': [], 'capture': [], 'type': ['text'],
                    'key': ['scancode']}
        if action not in required:
            raise ValueError(f'unknown action: {action}')
        for field in required[action]:
            if field not in step:
                raise ValueError(f"step {step.get('id')}: missing {field}")
            if field in ('at', 'from_xy', 'to_xy'):
                point = step[field]
                if not isinstance(point, list) or len(point) != 2 or not all(
                        isinstance(x, (int, float)) for x in point):
                    raise ValueError(f'invalid point: {field}')
        settle = step.get('settle_ms', defaults.get('settle_ms', 1500))
        hold = (step.get('duration_ms', 300) if action == 'swipe' else
                step.get('press_ms', defaults.get('press_ms', 120))
                if action in ('tap', 'key') else 0)
        if any(not isinstance(x, int) or x < 0 for x in (settle, hold)):
            raise ValueError('timings must be nonnegative integer milliseconds')
        duration += settle + hold
    return duration / 1000


def outcome(log, exit_code, elapsed, deadline, completed_at, settle):
    if any(marker in log for marker in ('panicked at', 'fatal runtime error',
                                         'tapHLE crashed')):
        return 'runtime_error'
    if exit_code is not None:
        return 'process_exited'
    if completed_at is not None and elapsed - completed_at >= settle:
        # Guest output shares this stream. A log marker is a capture hint,
        # never authoritative proof that the host finished the replay.
        return 'completion_unverified'
    if elapsed >= deadline:
        return 'timeout'
    return None


def simctl(*args, timeout=30):
    result = subprocess.run(['xcrun', 'simctl', *args], capture_output=True,
                            text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return result.stdout.strip()


def run_case(args, slug):
    result = {'slug': slug, 'visual_review': 'pending'}
    process = None
    started = time.monotonic()
    try:
        app = args.apps / (slug + '.ipa')
        route_path = args.maps / (slug + '.json')
        route = json.loads(route_path.read_text())
        deadline = validate_route(route) + args.startup_timeout + args.settle
        result.update(app_sha256=digest(app), route_sha256=digest(route_path),
                      options=route.get('options', []), deadline_seconds=deadline)
        recorded_hash = route.get('app', {}).get('sha256')
        result['recorded_artifact_match'] = (
            result['app_sha256'] == recorded_hash.lower() if recorded_hash else None)
        if recorded_hash and not result['recorded_artifact_match']:
            raise ValueError('artifact hash differs from recorded clickmap')
        log_path = args.output / (slug + '.log')
        with log_path.open('w') as log:
            process = subprocess.Popen(
                ['xcrun', 'simctl', 'launch', '--terminate-running-process',
                 '--console', args.device, args.bundle, str(app),
                 *route.get('options', []), '--no-error-popup',
                 *(['--replay-scale-to-viewport'] if args.scale_to_viewport else []),
                 '--replay=' + str(route_path)], stdout=log,
                stderr=subprocess.STDOUT,
                env={**os.environ, 'SIMCTL_CHILD_RUST_BACKTRACE': '1',
                     'SIMCTL_CHILD_TAPHLE_FRAME_CAPTURE_REQUEST':
                         str(args.output / (slug + '.capture-request')),
                     'SIMCTL_CHILD_TAPHLE_FRAME_CAPTURE_OUTPUT':
                         str(args.output / (slug + '.ppm'))})
            completed_at = None
            # Read incrementally; large guest logs must not be reread every tick.
            with log_path.open(errors='replace') as reader:
                tail = ''
                capture_index = 0
                while True:
                    chunk = reader.read()
                    tail = (tail + chunk)[-65536:]
                    # Keep failures even when one read contains a very large log.
                    if any(marker in chunk for marker in
                           ('panicked at', 'fatal runtime error', 'tapHLE crashed')):
                        tail += '\npanicked at (captured in log)'
                    if args.capture_steps and re.search(r'^Replay: .+ \(.*\)$', chunk, re.M):
                        capture_index += 1
                        simctl('io', args.device, 'screenshot', str(
                            args.output / f'{slug}-step-{capture_index:02}.png'))
                    elapsed = time.monotonic() - started
                    if 'Replay: all ' in tail and completed_at is None:
                        completed_at = elapsed
                        if args.guest_capture:
                            (args.output / (slug + '.capture-request')).touch()
                    status = outcome(tail, process.poll(), elapsed, deadline,
                                     completed_at, args.settle)
                    if status:
                        result['status'] = status
                        if status == 'runtime_error':
                            # Symbolication can be slow in a VM. Preserve the
                            # backtrace before terminating a panicked guest.
                            grace = time.monotonic() + 30
                            while process.poll() is None and time.monotonic() < grace:
                                tail = (tail + reader.read())[-65536:]
                                if 'Register state immediately after panic' in tail:
                                    break
                                time.sleep(1)
                        break
                    time.sleep(1)
            result['completion_log_observed'] = completed_at is not None
            result['replay_finished'] = None  # No trusted completion channel yet.
            result['process_exit'] = process.poll()
            if args.guest_capture:
                result['guest_frame_captured'] = (args.output / (slug + '.ppm')).exists()
            try:
                simctl('io', args.device, 'screenshot',
                       str(args.output / (slug + '.png')))
            except (RuntimeError, subprocess.TimeoutExpired) as error:
                result['capture_error'] = str(error)
                result['status'] = 'capture_failed'
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        result.update(status='harness_error', error=str(error))
    finally:
        if process is not None:
            try:
                simctl('terminate', args.device, args.bundle)
            except (RuntimeError, subprocess.TimeoutExpired):
                pass
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    result['seconds'] = round(time.monotonic() - started, 1)
    (args.output / (slug + '.json')).write_text(json.dumps(result, indent=2))
    return result


def cli_outcome(log, exit_code):
    matches = re.findall(r'Passed (\d+) out of (\d+) tests', log)
    if exit_code != 0 or not matches or 'panicked at' in log:
        return 'cli_failed'
    passed, total = map(int, matches[-1])
    return 'cli_passed' if passed == total and total > 0 else 'cli_failed'


def run_cli(args):
    result = {'slug': 'synthetic-cli', 'status': 'cli_failed'}
    log_path = args.output / 'synthetic-cli.log'
    process = None
    try:
        fixture = args.cli_fixture.resolve()
        with (fixture / 'Info.plist').open('rb') as stream:
            binary = plistlib.load(stream)['CFBundleExecutable']
        result['fixture_sha256'] = digest(fixture / binary)
        with log_path.open('w') as log:
            process = subprocess.Popen([
                'xcrun', 'simctl', 'launch', '--terminate-running-process',
                '--console', args.device, args.bundle, str(fixture),
                '--headless', '--no-error-popup', '--args', '--cli-tests'],
                stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + args.cli_timeout
            while time.monotonic() < deadline:
                text = log_path.read_text(errors='replace')
                # On mobile, guest exit returns to the library, so the host
                # process may remain alive after a successful CLI suite.
                if 'App called exit(), exiting.' in text or process.poll() is not None:
                    result['status'] = cli_outcome(text, process.poll() or 0)
                    break
                if 'panicked at' in text:
                    break
                time.sleep(1)
            else:
                result['error'] = 'synthetic CLI timeout'
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        result['error'] = str(error)
    finally:
        try:
            simctl('terminate', args.device, args.bundle)
        except (RuntimeError, subprocess.TimeoutExpired):
            pass
        if process is not None:
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    (args.output / 'synthetic-cli.json').write_text(json.dumps(result, indent=2))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('slugs', nargs='*', help='explicit complete case selection')
    parser.add_argument('--device', required=True, help='booted simulator UUID')
    parser.add_argument('--bundle', default='org.taphle.ios')
    parser.add_argument('--apps', type=Path, default=ROOT / 'runtime/apps')
    parser.add_argument('--maps', type=Path, default=ROOT / 'compatibility/clickmaps')
    parser.add_argument('--output', type=Path, required=True,
                        help='new evidence directory; existing runs are never overwritten')
    parser.add_argument('--startup-timeout', type=float, default=100)
    parser.add_argument('--settle', type=float, default=15)
    parser.add_argument('--capture-steps', action='store_true',
                        help='also capture at replay step transitions for milestone review')
    parser.add_argument('--scale-to-viewport', action='store_true',
                        help='explicitly adapt recorded desktop coordinates to this viewport')
    parser.add_argument('--guest-capture', action='store_true',
                        help='capture submitted GLES frame after replay for presentation diagnosis')
    parser.add_argument('--cli-fixture', type=Path,
                        help='also run a built TestApp.app synthetic CLI suite')
    parser.add_argument('--cli-timeout', type=float, default=120)
    args = parser.parse_args()
    if not args.slugs and not args.cli_fixture:
        parser.error('select app routes or a synthetic CLI fixture')
    if len(set(args.slugs)) != len(args.slugs) or any(
            Path(slug).name != slug or slug in ('.', '..') for slug in args.slugs):
        parser.error('case names must be distinct simple filenames')
    if args.startup_timeout <= 0 or args.settle < 0:
        parser.error('invalid timeout or settle duration')
    args.apps = args.apps.resolve()
    args.maps = args.maps.resolve()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    bundle = Path(simctl('get_app_container', args.device, args.bundle, 'app'))
    with (bundle / 'Info.plist').open('rb') as stream:
        executable = plistlib.load(stream)['CFBundleExecutable']
    provenance = {'device': args.device, 'bundle': args.bundle,
                  'installed_binary_sha256': digest(bundle / executable),
                  'selected_cases': args.slugs,
                  'scale_to_viewport': args.scale_to_viewport,
                  'source_commit': subprocess.check_output(
                      ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                  'source_dirty': bool(subprocess.check_output(
                      ['git', 'status', '--porcelain'], cwd=ROOT)),
                  'note': 'Source state is not proof of installed binary provenance. '
                          'Console completion markers are unverified; app routes '
                          'require a trusted host completion channel before they '
                          'can pass automatically. No gameplay or release pass is inferred.'}
    (args.output / 'provenance.json').write_text(json.dumps(provenance, indent=2))
    results = []
    if args.cli_fixture:
        result = run_cli(args)
        results.append(result)
        print(f"synthetic-cli: {result['status']}", flush=True)
        (args.output / 'results.json').write_text(json.dumps(results, indent=2))
    for slug in args.slugs:
        result = run_case(args, slug)
        results.append(result)
        print(f"{slug}: {result['status']} ({result['seconds']}s)", flush=True)
        (args.output / 'results.json').write_text(json.dumps(results, indent=2))
    # A log-only app-route observation must fail closed. Synthetic CLI runs
    # use the selected, hashed test fixture rather than arbitrary guest apps.
    return int(any(r['status'] != 'cli_passed' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
