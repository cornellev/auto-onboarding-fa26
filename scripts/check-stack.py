"""Run real transport checks against isolated loopback services, then clean up."""
import argparse,json,os,shutil,socket,subprocess,sys,tempfile,time,urllib.request
from pathlib import Path

def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1',0))
        return s.getsockname()[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--orchestrator',type=Path,required=True)
    p.add_argument('--sim',type=Path,required=True)
    p.add_argument('--router',default=shutil.which('zenohd'))
    args=p.parse_args()
    if not args.router:p.error('Install zenohd 1.10.2 or pass --router')
    root=Path(__file__).resolve().parents[1]
    router_port,ws_port,api_port=port(),port(),port()
    env={**os.environ,'WS_PORT':str(ws_port),'API_PORT':str(api_port),'WS_HOST':'127.0.0.1','API_HOST':'127.0.0.1',
         'CEV_ORCHESTRATOR_API':f'http://127.0.0.1:{api_port}','CEV_ORCHESTRATOR_WS':f'ws://127.0.0.1:{ws_port}',
         'CEV_ZENOH_ENDPOINT':f'tcp/127.0.0.1:{router_port}','ROS_ENABLED':'false','ZENOH_ENABLED':'false'}
    processes=[]
    with tempfile.TemporaryDirectory(prefix='cev-protobuf-check-') as temp:
        temp=Path(temp);logs=[]
        def start(cmd,cwd,name):
            log=open(temp/(name+'.log'),'w');logs.append(log)
            proc=subprocess.Popen(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
            processes.append(proc)
            return proc
        def wait_port(number,proc):
            # shortcut: orchestrator zenoh.open still leaves shared memory on; if macOS still times out, disable it in cornellev/orchestrator and bump orchestrator_ref
            deadline=time.monotonic()+45
            while time.monotonic()<deadline:
                if proc.poll() is not None:raise RuntimeError('Service exited during startup')
                try:
                    with socket.create_connection(('127.0.0.1',number),timeout=.2):return
                except OSError:time.sleep(.05)
            raise TimeoutError('Service startup timed out')
        try:
            config=temp/'router.json5'
            config.write_text(json.dumps({'mode':'router','listen':{'endpoints':[f'tcp/127.0.0.1:{router_port}']},
                'scouting':{'multicast':{'enabled':False},'gossip':{'enabled':False}},'plugins_loading':{'enabled':False},
                'transport':{'shared_memory':{'enabled':False}}}))
            router=start([str(Path(args.router).resolve()),'-c',str(config)],root,'router')
            wait_port(router_port,router)
            orch=start([sys.executable,'-u','main.py','--zenoh-profile','interop','--zenoh-connect',env['CEV_ZENOH_ENDPOINT']],args.orchestrator,'orchestrator')
            wait_port(ws_port,orch);wait_port(api_port,orch)
            subprocess.run(['cargo','build','--locked','--examples'],cwd=root,env=env,check=True,timeout=600)
            subprocess.run([sys.executable,str(args.orchestrator/'scripts/check_protobuf_live.py'),'--onboarding',str(root),'--sim',str(args.sim)],env=env,check=True,timeout=30)
            subprocess.run(['cargo','test','--locked','--lib','http_authorization','--','--ignored'],cwd=root,env=env,check=True,timeout=30)
            subprocess.run(['cargo','test','--locked','--test','wiring','three_processes','--','--ignored'],cwd=root,env=env,check=True,timeout=60)
        except Exception:
            for log in logs:
                log.flush();print(Path(log.name).read_text(errors='replace'),file=sys.stderr)
            raise
        finally:
            for proc in reversed(processes):
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            for log in logs:log.close()
    print('PASS: isolated generic routing and three-process bootstrap')

if __name__=='__main__':main()
