#!/usr/bin/env python3
"""Serve the compiled Leptos app; proxy binary RPC without touching its payload."""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
import os
import ssl
import threading
import urllib.request
import urllib.error
ROOT=Path(__file__).resolve().parents[1]
BACKEND='http://127.0.0.1:'+os.environ.get('CAFE_PORT','8788')
class CafeHTTPServer(ThreadingHTTPServer):
    request_queue_size = 128
    daemon_threads = True

class Handler(SimpleHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    timeout = 30
    def handle(self):
        try: super().handle()
        except (BrokenPipeError, ConnectionResetError): self.close_connection = True
    def __init__(self,*args,**kwargs): super().__init__(*args,directory=str(ROOT/'dist'),**kwargs)
    def end_headers(self):
        self.send_header('Cache-Control','no-store')
        if isinstance(self.connection, ssl.SSLSocket):
            self.send_header('Alt-Svc', 'h3="'+os.environ.get('CAFE_HTTP3_AUTHORITY', ':'+os.environ.get('CAFE_HTTPS_PORT','8792'))+'"; ma=60')
        super().end_headers()
    def do_POST(self):
        if self.path=='/api/events': self.events(); return
        if self.path!='/api/rpc': self.send_error(404); return
        self.proxy()
    def do_GET(self):
        if self.path == '/local-ca.cer' and os.environ.get('CAFE_LOCAL_CA'):
            data=Path(os.environ['CAFE_LOCAL_CA']).read_bytes()
            self.send_response(200)
            self.send_header('Content-Type','application/x-x509-ca-cert')
            self.send_header('Content-Length',str(len(data)))
            self.end_headers(); self.wfile.write(data); return
        if self.path.startswith('/media/') or self.path.split('?',1)[0] in ('/qr','/health'): self.proxy()
        else: super().do_GET()
    def events(self):
        # Stream notifications without buffering or logging the bearer token.
        self.close_connection = True
        try:
            request=urllib.request.Request(BACKEND+'/api/events',data=b'',method='POST',
                headers={'Authorization':self.headers.get('Authorization','')})
            with urllib.request.urlopen(request,timeout=75) as response:
                self.send_response(200)
                self.send_header('Content-Type','text/event-stream')
                self.send_header('Connection','close')
                self.send_header('X-Accel-Buffering','no')
                self.end_headers()
                while True:
                    line=response.readline()
                    if not line: break
                    self.wfile.write(line)
                    self.wfile.flush()
        except urllib.error.HTTPError as e: self.send_error(e.code)
        except (OSError,ValueError): pass

    def proxy(self):
        try:
            length=int(self.headers.get('Content-Length','0'))
            if length<0 or length>8*1024*1024: self.send_error(413); return
            body=self.rfile.read(length) if self.command=='POST' else None
            request=urllib.request.Request(BACKEND+self.path,data=body,method=self.command,headers={'Content-Type':self.headers.get('Content-Type','application/x-capnp')})
            with urllib.request.urlopen(request,timeout=70) as response:
                data=response.read(); self.send_response(response.status)
                self.send_header('Content-Type',response.headers.get('Content-Type','application/octet-stream'))
                self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            self.close_connection = True
        except urllib.error.HTTPError as e: self.send_error(e.code)
        except (OSError,ValueError): self.send_error(502,'Cafe backend is unavailable')
    def log_message(self,*args): pass
if __name__=='__main__':
    address=(os.environ.get('CAFE_FRONTEND_BIND','0.0.0.0'),int(os.environ.get('CAFE_FRONTEND_PORT','8791')))
    server=CafeHTTPServer(address,Handler)
    print(f'Leptos frontend http://{address[0]}:{address[1]}',flush=True)
    cert,key=os.environ.get('CAFE_TLS_CERT'),os.environ.get('CAFE_TLS_KEY')
    if cert and key:
        secure=CafeHTTPServer((address[0],int(os.environ.get('CAFE_HTTPS_PORT','8792'))),Handler)
        context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version=ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(cert,key)
        context.set_alpn_protocols(['http/1.1'])
        secure.socket=context.wrap_socket(secure.socket,server_side=True)
        threading.Thread(target=secure.serve_forever,daemon=True).start()
        print(f'HTTPS frontend https://{address[0]}:{secure.server_port}',flush=True)
    server.serve_forever()
