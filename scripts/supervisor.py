#!/usr/bin/env python3
"""Own and supervise only this checkout's cafe processes; no global kill commands."""
import argparse, json, os, signal, subprocess, sys, time, urllib.request, shlex, threading, ssl
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
STATE = Path(os.environ.get('CAFE_HOME', str(Path.home() / '.causewaybaycafe')))
RUN = STATE / 'run'
PID = RUN / 'supervisor.pid'
STOP = False
WAKE = threading.Event()

def alive(pid):
    try: os.kill(pid, 0); return True
    except ProcessLookupError: return False

def shutdown(*_):
    global STOP
    STOP = True
    WAKE.set()

def serve(simulation, mode=None):
    RUN.mkdir(parents=True, exist_ok=True)
    # O_EXCL prevents two supervisors from racing into the same checkout.
    try: fd = os.open(PID, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError:
        raise SystemExit('Supervisor already running or stale PID; run make stop first.')
    with os.fdopen(fd, 'w') as f: f.write(str(os.getpid()))
    signal.signal(signal.SIGTERM, shutdown); signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGCHLD, lambda *_: WAKE.set())
    env = dict(os.environ, NO_COLOR='true')
    if mode: env['CAFE_START_MODE'] = mode
    # Read only public process settings. The Rust backend loads all secrets itself.
    if (ROOT/'.env').exists():
        for line in (ROOT/'.env').read_text().splitlines():
            key,sep,value=line.partition('='); key=key.strip().removeprefix('export ')
            if sep and key in ('CAFE_PORT','CAFE_FRONTEND_PORT','CAFE_BIND','CAFE_FRONTEND_BIND','CAFE_GUEST_AI','CAFE_MANAGEMENT_AI') and key not in env:
                values=shlex.split(value,comments=True)
                if values: env[key]=values[0]
    env.setdefault('CAFE_GUEST_AI','1')
    env.setdefault('CAFE_MANAGEMENT_AI','1')
    env.setdefault('CAFE_BIND','0.0.0.0');env.setdefault('CAFE_FRONTEND_BIND','0.0.0.0')
    env.setdefault('CAFE_PORT','8788');env.setdefault('CAFE_FRONTEND_PORT','8791')
    # Opt in by generating local certificates with make tls. Preserve explicit overrides.
    tls = STATE / 'tls'
    if (tls/'server.pem').exists() and (tls/'server.key').exists():
        env.setdefault('CAFE_TLS_CERT',str(tls/'server.pem'))
        env.setdefault('CAFE_TLS_KEY',str(tls/'server.key'))
        env.setdefault('CAFE_HTTPS_PORT','8792')
        env.setdefault('CAFE_HTTP3_ADDR',env['CAFE_BIND']+':'+env['CAFE_HTTPS_PORT'])
        env.setdefault('CAFE_LOCAL_CA',str(tls/'causewaybay-ca.cer'))
    env['CAFE_URL']='http://127.0.0.1:'+env['CAFE_PORT']
    if simulation: env.update(CAFE_BIND='127.0.0.1', CAFE_FRONTEND_BIND='127.0.0.1', CAFE_SIMULATION='1', CAFE_DATABASE=str(STATE/'simulation.sqlite'), CAFE_OWNER_WALLET='0x9858effd232b4033e47d90003d41ec34ecaeda94')
    children = []
    logs = []
    try:
        for name, cmd in [('backend', [str(ROOT/'target/debug/cafe-server')]), ('frontend', [sys.executable,str(ROOT/'scripts/frontend.py')])]:
            log = open(RUN/f'{name}.log', 'a'); logs.append(log)
            children.append(subprocess.Popen(cmd, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT))
        if simulation:
            for _ in range(100):
                if STOP: break
                try:
                    urllib.request.urlopen(env['CAFE_URL']+'/health', timeout=.5); break
                except Exception: time.sleep(.1)
            log = open(RUN/'simulation.log', 'a'); logs.append(log)
            children.append(subprocess.Popen([str(ROOT/'target/debug/simulate')], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT))
        (RUN/'children.json').write_text(json.dumps([p.pid for p in children]))
        (RUN/'endpoints.json').write_text(json.dumps({'frontend':'http://127.0.0.1:'+env['CAFE_FRONTEND_PORT'],'backend':env['CAFE_URL'], 'https':('https://127.0.0.1:'+env['CAFE_HTTPS_PORT']) if env.get('CAFE_TLS_CERT') else None, 'ca':str(tls/'ca.pem')}))
        while not STOP:
            if any(p.poll() is not None for p in children[:2]):
                print('A required process exited. Stopping the café; inspect ~/.causewaybaycafe/run/*.log.', flush=True); break
            # Process health checks are independent of the AI work interval.
            WAKE.wait(1)
            WAKE.clear()
    finally:
        for p in children:
            if p.poll() is None: p.terminate()
        for p in children:
            try: p.wait(timeout=8)
            except subprocess.TimeoutExpired: p.kill(); p.wait()
        for f in logs: f.close()
        PID.unlink(missing_ok=True)
        (RUN/'children.json').unlink(missing_ok=True)
        (RUN/'endpoints.json').unlink(missing_ok=True)

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('command', choices=['start','stop','status','serve']);parser.add_argument('--simulation',action='store_true');parser.add_argument('--mode',choices=['simulation','live']);args=parser.parse_args();os.chdir(ROOT);RUN.mkdir(parents=True, exist_ok=True)
    if args.command=='serve': return serve(args.simulation, args.mode)
    pid=int(PID.read_text()) if PID.exists() else None
    if args.command=='status': print(f'Running (supervisor {pid})' if pid and alive(pid) else 'Stopped');return
    if args.command=='stop':
        if pid and alive(pid):
            # Verify PID belongs to our supervisor before signalling a reused PID.
            command=subprocess.check_output(['ps','-p',str(pid),'-o','command='],text=True)
            if str(Path(__file__).resolve()) not in command or 'serve' not in command: raise SystemExit('PID belongs to another process; refusing to signal it.')
            os.kill(pid,signal.SIGTERM)
            for _ in range(120):
                if not PID.exists(): break
                time.sleep(.1)
        else: PID.unlink(missing_ok=True)
        print('Cafe stopped.');return
    if pid and alive(pid): print('Cafe already running.');return
    PID.unlink(missing_ok=True)
    (RUN/'endpoints.json').unlink(missing_ok=True)
    with open(RUN/'supervisor.log','a') as log:
        cmd=[sys.executable,str(Path(__file__).resolve()),'serve']+(['--simulation'] if args.simulation else [])+(['--mode',args.mode] if args.mode else [])
        p=subprocess.Popen(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    last_error = None
    for _ in range(100):
        if p.poll() is not None: raise SystemExit('Startup failed; see ~/.causewaybaycafe/run/supervisor.log')
        try:
            endpoints=json.loads((RUN/'endpoints.json').read_text())
            urllib.request.urlopen(endpoints['frontend'], timeout=.5)
            urllib.request.urlopen(endpoints['backend']+'/health', timeout=.5)
            if endpoints.get('https'):
                context=ssl.create_default_context(cafile=endpoints['ca'])
                urllib.request.urlopen(endpoints['https']+'/health',context=context,timeout=1)
                print(f"HTTPS + HTTP/3: {endpoints['https']} (trust the local CA on your device)")
            print(f"Cafe ready: {endpoints['frontend']} (frontend) · {endpoints['backend']} (backend + built UI)");return
        except Exception as error:
            last_error = error
            time.sleep(.2)
    raise SystemExit(f'Startup health check failed: {last_error}. Inspect ~/.causewaybaycafe/run/*.log and make status.')
if __name__=='__main__': main()
