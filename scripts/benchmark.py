"""Isolated, reproducible HTTP archive benchmark. Never opens the user's database."""
import contextlib, http.client, sys
import argparse, ctypes, datetime, json, os, pathlib, socket, sqlite3, subprocess, tempfile, time

connections = {}
def request(base, path, payload=None):
    # Reuse HTTP/1.1 connections so Windows ephemeral ports do not dominate the run.
    connection = connections.setdefault(base, http.client.HTTPConnection(base.removeprefix('http://'), timeout=120))
    body = None if payload is None else json.dumps(payload).encode()
    try:
        connection.request('GET' if payload is None else 'POST', path, body=body, headers={"Content-Type":"application/json"})
        response = connection.getresponse(); raw=response.read()
        if response.status >= 400: raise RuntimeError(f'{response.status}: {raw[:1000]!r}')
        return json.loads(raw),len(raw)
    except Exception:
        # Failed startup connections cannot be reused in the next readiness probe.
        connections.pop(base, None)
        connection.close()
        raise

def peak_rss(child):
    if os.name == 'nt':
        from ctypes import wintypes
        class Counters(ctypes.Structure):
            _fields_ = [("cb",wintypes.DWORD),("PageFaultCount",wintypes.DWORD)] + [(n,ctypes.c_size_t) for n in ["PeakWorkingSetSize","WorkingSetSize","QuotaPeakPagedPoolUsage","QuotaPagedPoolUsage","QuotaPeakNonPagedPoolUsage","QuotaNonPagedPoolUsage","PagefileUsage","PeakPagefileUsage"]]
        counters = Counters(); counters.cb = ctypes.sizeof(counters)
        fn = ctypes.windll.psapi.GetProcessMemoryInfo
        fn.argtypes = [wintypes.HANDLE,ctypes.c_void_p,wintypes.DWORD]
        if fn(int(child._handle),ctypes.byref(counters),counters.cb): return counters.PeakWorkingSetSize
    return None

def run(exe, count, check_backup=False):
    with tempfile.TemporaryDirectory(prefix='harnesscope-benchmark-') as name:
        root = pathlib.Path(name); db = root / 'bench.db'
        with socket.socket() as sock:
            sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
        env = dict(os.environ,HARNESSCOPE_DB_PATH=str(db),HARNESSCOPE_DATA_DIR=str(root),HARNESSCOPE_AUTOSTART='0',HARNESSCOPE_SERVER_URL=f'http://127.0.0.1:{port}')
        log = open(root / 'server.log', 'w+b')
        child = subprocess.Popen([str(exe),'serve','--host','127.0.0.1','--port',str(port)],env=env,stdout=log,stderr=log)
        base = f'http://127.0.0.1:{port}'
        try:
            deadline=time.monotonic()+20
            while True:
                try: request(base,'/api/v1/health'); break
                except Exception:
                    if time.monotonic()>deadline: raise
                    time.sleep(.05)
            stamp=datetime.datetime.now(datetime.timezone.utc).isoformat()
            def event(i,kind,payload,execution=True):
                return dict(event_id=f'bench-{i}',timestamp=stamp,event_type=kind,source='synthetic-benchmark',runtime_id='bench-run',session_id='bench-session',execution_id='bench-execution' if execution else None,payload=payload)
            initial=[event('run','runtime.started',dict(runner_name='benchmark',cwd=str(root)),False),event('session','session.identified',dict(runner_name='benchmark',native_session_id='benchmark-thread'),False),event('exec','execution.started',dict(capture_scope='TURN',model='synthetic'))]
            request(base,'/api/v1/delivery',initial)
            start=time.perf_counter()
            for offset in range(0,count,500):
                batch=[event(i,'benchmark.observed',dict(sequence=i,text='x'*128)) for i in range(offset,min(offset+500,count))]
                ack,_=request(base,'/api/v1/delivery',batch)
                assert ack['event_ids']==[e['event_id'] for e in batch]
                if (offset+500)%10000==0: print(f'{count}: imported {offset+500}',flush=True)
            elapsed=time.perf_counter()-start
            # Duplicate acknowledgements must not add rows.
            request(base,'/api/v1/delivery',batch)
            with contextlib.closing(sqlite3.connect(db)) as conn:
                stored=conn.execute('SELECT COUNT(*) FROM events').fetchone()[0]
                assert stored==count+3,(stored,count)
            timings={}
            for path in ['/api/v1/health','/api/v1/executions','/api/v1/retrospective','/api/v1/executions/bench-execution','/api/v1/sessions/bench-session']:
                samples=[]
                for _ in range(5):
                    t=time.perf_counter(); _,size=request(base,path); samples.append((time.perf_counter()-t)*1000)
                timings[path]={'median_ms':sorted(samples)[2],'max_ms':max(samples),'response_bytes':size}
            result=dict(events=count,stored_events=stored,import_seconds=elapsed,events_per_second=count/elapsed,api=timings,server_peak_rss_bytes=peak_rss(child),db_and_wal_bytes=sum(p.stat().st_size for p in root.glob('bench.db*')),profile='single long execution, 128-byte synthetic payload, 500-event atomic batches, debug binary')
            if check_backup:
                # Snapshot the live WAL database; restore while the original server remains open.
                start=time.perf_counter()
                subprocess.run([str(exe),'backup','create','--output',str(root/'snapshot.db')],env=env,capture_output=True,check=True)
                restored=subprocess.run([str(exe),'backup','restore','--file',str(root/'snapshot.db'),'--output',str(root/'restored.db')],env=env,capture_output=True,check=True)
                report=json.loads(restored.stdout); assert report['events']==stored
                result['backup_and_verified_restore_seconds']=time.perf_counter()-start
                result['restored_events']=report['events']
                page,size=request(base,'/api/v1/executions/bench-execution/events')
                assert len(page['items'])==min(50,count+1) and page['total']==count+1
                result['event_page_bytes']=size
            request(base,'/api/v1/shutdown',{}); child.wait(timeout=15)
            result['database_bytes']=db.stat().st_size
            return result
        except BaseException:
            log.flush(); log.seek(0); print(log.read().decode('utf8',errors='replace')[-10000:],file=sys.stderr)
            raise
        finally:
            if base in connections: connections.pop(base).close()
            if child.poll() is None: child.kill(); child.wait()
            log.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--exe',required=True,type=pathlib.Path); parser.add_argument('--sizes',default='10000,100000'); parser.add_argument('--output',required=True,type=pathlib.Path); parser.add_argument('--check-backup',action='store_true')
    args=parser.parse_args(); results=[]
    for count in map(int,args.sizes.split(',')):
        assert 1<=count<=1000000
        results.append(run(args.exe.resolve(),count,args.check_backup)); args.output.parent.mkdir(parents=True,exist_ok=True); args.output.write_text(json.dumps(results,indent=2),encoding='utf8'); print(json.dumps(results[-1]),flush=True)
