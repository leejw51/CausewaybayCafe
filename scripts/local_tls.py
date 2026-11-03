#!/usr/bin/env python3
"""Create a private local test CA and a SAN certificate; never install trust silently."""
import argparse, ipaddress, os, subprocess
from pathlib import Path

def run(*args):
    subprocess.run(['openssl', *map(str,args)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--host', action='append', default=[])
    args=parser.parse_args()
    os.umask(0o077)
    folder=Path(os.environ.get('CAFE_HOME',str(Path.home()/'.causewaybaycafe')))/'tls'
    folder.mkdir(parents=True,exist_ok=True)
    ca,ca_key=folder/'ca.pem',folder/'ca.key'
    if not ca.exists():
        run('req','-x509','-newkey','rsa:3072','-nodes','-sha256','-days','3650','-keyout',ca_key,'-out',ca,'-subj','/CN=Causewaybay Cafe Local Test CA','-addext','basicConstraints=critical,CA:TRUE,pathlen:0','-addext','keyUsage=critical,keyCertSign,cRLSign')
    if not ca_key.exists(): raise SystemExit('CA private key missing; restore it before issuing a certificate.')
    hosts=list(dict.fromkeys(['localhost','127.0.0.1','::1']+args.host))
    san=[]
    for host in hosts:
        try: san.append('IP:'+str(ipaddress.ip_address(host)))
        except ValueError:
            if not host or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-' for c in host): raise SystemExit('Invalid hostname')
            san.append('DNS:'+host)
    ext=folder/'server.ext'
    ext.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid,issuer\nsubjectAltName='+','.join(san)+'\n')
    run('req','-new','-newkey','rsa:2048','-nodes','-keyout',folder/'server.key','-out',folder/'server.csr','-subj','/CN=localhost')
    run('x509','-req','-in',folder/'server.csr','-CA',ca,'-CAkey',ca_key,'-CAcreateserial','-out',folder/'server.pem','-days','365','-sha256','-extfile',ext)
    run('x509','-in',ca,'-outform','DER','-out',folder/'causewaybay-ca.cer')
    run('verify','-CAfile',ca,folder/'server.pem')
    print(f'Local TLS ready in {folder}; hosts: {", ".join(hosts)}')
    print('Install only causewaybay-ca.cer on test devices. Private keys stay on this Mac.')
if __name__=='__main__': main()
