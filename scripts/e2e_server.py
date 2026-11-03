#!/usr/bin/env python3
"""Disposable, loopback-only test cafe. Never reads/writes the real cafe database."""
import argparse, os, signal, subprocess, tempfile, time, urllib.request, sqlite3, json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--scenario', choices=['simulation', 'stress'])
parser.add_argument('--live-demo', action='store_true', help='Explicit paid Meta guest demo with a disposable database')
args = parser.parse_args()
children = []
stopping = False

def stop(*_):
    global stopping
    stopping = True
    for child in children:
        if child.poll() is None: child.terminate()

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
with tempfile.TemporaryDirectory(prefix='cafe-e2e-') as folder:
    env = dict(os.environ, CAFE_BIND='127.0.0.1', CAFE_FRONTEND_BIND='127.0.0.1',
               CAFE_PORT='18888', CAFE_FRONTEND_PORT='18890', CAFE_DATABASE=str(Path(folder)/'test.sqlite'),
               CAFE_SIMULATION='1', CAFE_GUEST_AI='0', CAFE_MANAGEMENT_AI='0', CAFE_OWNER_WALLET='0x9858effd232b4033e47d90003d41ec34ecaeda94',
               CAFE_PUBLIC_URL='http://127.0.0.1:18890', CAFE_TLS_CERT='', CAFE_TLS_KEY='',
               MUSE_API_KEY='', OPENAI_API_KEY='', XAI_API_KEY='')
    if args.live_demo:
        env['CAFE_GUEST_AI']='1'
        # Rust loads the configured key from .env; never print or copy it into artifacts.
        if os.environ.get('MUSE_API_KEY'): env['MUSE_API_KEY']=os.environ['MUSE_API_KEY']
        else: env.pop('MUSE_API_KEY',None)
    try:
        children.append(subprocess.Popen([str(ROOT/'target/debug/cafe-server')], cwd=ROOT, env=env))
        deadline = time.monotonic()+20
        while time.monotonic()<deadline and not stopping:
            if children[0].poll() is not None: raise RuntimeError('Test backend exited')
            try:
                urllib.request.urlopen('http://127.0.0.1:18888/health', timeout=.5)
                break
            except OSError: time.sleep(.1)
        else: raise RuntimeError('Test backend failed to start')
        children.append(subprocess.Popen(['python3',str(ROOT/'scripts/frontend.py')], cwd=ROOT, env=env))
        if args.scenario:
            count = 12 if args.scenario == 'simulation' else 64
            concurrency = 1 if args.scenario == 'simulation' else 8
            env.update(CAFE_URL='http://127.0.0.1:18888', CAFE_TEST_ORDERS=str(count),
                       CAFE_TEST_CONCURRENCY=str(concurrency), CAFE_TEST_DELAY_MS='250' if args.scenario == 'simulation' else '0')
            worker = subprocess.Popen([str(ROOT/'target/debug/simulate')], cwd=ROOT, env=env)
            children.append(worker)
            code = worker.wait(timeout=180)
            if code: raise RuntimeError(f'Automatic scenario failed: exit {code}')
            with sqlite3.connect(Path(folder)/'test.sqlite') as db:
                orders, completed, total = db.execute("SELECT COUNT(*),SUM(status='completed'),SUM(total) FROM orders").fetchone()
                payments, paid = db.execute("SELECT COUNT(*),SUM(amount) FROM payments WHERE kind='payment'").fetchone()
                assert (orders, completed, payments) == (count, count, count), 'Incomplete or duplicate orders/payments'
                assert total == paid, 'Payment total mismatch'
                assert db.execute('SELECT COUNT(*) FROM ai_usage').fetchone()[0] == 0, 'Unexpected AI usage'
                assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
                journal = [json.loads(line) for line in (Path(folder)/'test.jsonl').read_text().splitlines()]
                assert len(journal) == db.execute('SELECT COUNT(*) FROM persistence_events').fetchone()[0], 'Unflushed journal'
            print(f'PASS: {count} orders completed and paid, SQLite/JSONL verified, zero AI calls.', flush=True)
        while not stopping and not args.scenario:
            if any(p.poll() is not None for p in children): raise RuntimeError('Test service exited')
            time.sleep(.5)
    finally:
        stop()
        for child in children:
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired: child.kill();child.wait()
