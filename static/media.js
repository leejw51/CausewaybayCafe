let recognition;
let audioStream, audioContext, stopAudio, cameraStream, cameraVideo, audioEpoch=0;
export function stop_listening() { if(recognition){recognition.abort();recognition=null;} audioEpoch++; if(stopAudio) stopAudio(); if(audioStream) audioStream.getTracks().forEach(t=>t.stop()); }
export async function listen_once() {
  const epoch=audioEpoch;
  if(!navigator.mediaDevices) throw new Error('Microphone needs HTTPS or localhost.');
  audioStream=await navigator.mediaDevices.getUserMedia({audio:{channelCount:1,echoCancellation:true,noiseSuppression:true}});
  if(epoch!==audioEpoch){audioStream.getTracks().forEach(t=>t.stop());return '';}
  audioContext=new AudioContext({sampleRate:24000}); await audioContext.resume();
  const source=audioContext.createMediaStreamSource(audioStream), processor=audioContext.createScriptProcessor(4096,1,1);
  const frames=[]; let length=0, voiced=false, silence=0, finished=false;
  return await new Promise((resolve,reject)=>{
    const finish=()=>{if(finished)return;finished=true;clearTimeout(timeout);processor.disconnect();source.disconnect();audioStream.getTracks().forEach(t=>t.stop());const rate=audioContext.sampleRate;audioContext.close();stopAudio=null;
      if(!voiced){resolve('');return;}
      const buffer=new ArrayBuffer(44+length*2), view=new DataView(buffer); const str=(p,s)=>[...s].forEach((c,i)=>view.setUint8(p+i,c.charCodeAt(0)));
      str(0,'RIFF');view.setUint32(4,36+length*2,true);str(8,'WAVE');str(12,'fmt ');view.setUint32(16,16,true);view.setUint16(20,1,true);view.setUint16(22,1,true);view.setUint32(24,rate,true);view.setUint32(28,rate*2,true);view.setUint16(32,2,true);view.setUint16(34,16,true);str(36,'data');view.setUint32(40,length*2,true);
      let offset=44;for(const frame of frames) for(const x of frame){view.setInt16(offset,Math.max(-1,Math.min(1,x))*(x<0?32768:32767),true);offset+=2;}
      const bytes=new Uint8Array(buffer);let binary='';for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));resolve(btoa(binary));};
    stopAudio=finish; const timeout=setTimeout(finish,20000);
    processor.onaudioprocess=e=>{const f=new Float32Array(e.inputBuffer.getChannelData(0));frames.push(f);length+=f.length;const rms=Math.sqrt(f.reduce((s,x)=>s+x*x,0)/f.length);if(rms>.018){voiced=true;silence=0;}else{silence+=f.length/audioContext.sampleRate;}if(voiced&&silence>1.3)finish();};
    source.connect(processor);processor.connect(audioContext.destination);
  });
}
export async function camera_start(){if(!navigator.mediaDevices)throw new Error('Camera needs HTTPS or localhost.');cameraStream=await navigator.mediaDevices.getUserMedia({video:{facingMode:'environment',width:640},audio:false});const v=document.getElementById('camera-preview')||document.createElement('video');v.muted=true;v.playsInline=true;cameraVideo=v;v.srcObject=cameraStream;await v.play();}
export function camera_stop(){if(cameraStream)cameraStream.getTracks().forEach(t=>t.stop());cameraStream=null;}
export function camera_frame(){const v=cameraVideo, audioEpoch=0;if(!cameraStream||!v||!v.videoWidth)return '';const c=document.createElement('canvas');c.width=640;c.height=640*v.videoHeight/v.videoWidth;c.getContext('2d').drawImage(v,0,0,c.width,c.height);return c.toDataURL('image/jpeg',.7);}
window.addEventListener('pagehide',()=>{stop_listening();camera_stop();});
export function print_page(){window.print();}
export async function read_menu_photo(){return read_photo("menu-photo-input");}
export async function read_floor_photo(){return read_photo("floor-photo-input");}
async function read_photo(id){
 const file=document.getElementById(id)?.files?.[0];if(!file)throw new Error('Choose a photo first.');
 if(!file.type.startsWith('image/')||file.size>15*1024*1024)throw new Error('Choose an image smaller than 15 MB.');
 const url=URL.createObjectURL(file);try{const img=new Image();img.src=url;await img.decode();const scale=Math.min(1,1600/Math.max(img.width,img.height));const c=document.createElement('canvas');c.width=Math.round(img.width*scale);c.height=Math.round(img.height*scale);c.getContext('2d').drawImage(img,0,0,c.width,c.height);const data=c.toDataURL('image/jpeg',.82);if(data.length>2000000)throw new Error('Photo is too detailed. Crop it into smaller sections.');return data;}finally{URL.revokeObjectURL(url);}
}
// A persistent server event stream; no timer fetches order state.
let updatesController;
export function unsubscribe_updates(){updatesController?.abort();updatesController=null;}
export function subscribe_updates(token,onChange){
 unsubscribe_updates();if(!token)return;
 const controller=new AbortController();updatesController=controller;
 (async()=>{let delay=1000;
  while(!controller.signal.aborted){
   try{
    const response=await fetch('/api/events',{method:'POST',headers:{Authorization:`Bearer ${token}`},signal:controller.signal});
    if(response.status===401||response.status===403)return;
    if(!response.ok||!response.body)throw new Error('Event stream unavailable');
    delay=1000;const reader=response.body.getReader();const decoder=new TextDecoder();let buffer='';
    try{while(!controller.signal.aborted){const {value,done}=await reader.read();if(done)break;buffer+=decoder.decode(value,{stream:true});let end;
     while((end=buffer.indexOf('\n\n'))!==-1){const event=buffer.slice(0,end);buffer=buffer.slice(end+2);if(event.split('\n').some(line=>line==='data: refresh'))onChange();}
    }}finally{await reader.cancel().catch(()=>{});reader.releaseLock();}
   }catch(error){if(controller.signal.aborted)return;}
   // Reconnect only after disconnection. This is not order polling.
   await new Promise(resolve=>{const finish=()=>{clearTimeout(timer);controller.signal.removeEventListener('abort',finish);resolve();};const timer=setTimeout(finish,delay);controller.signal.addEventListener('abort',finish,{once:true});});
   delay=Math.min(delay*2,30000);
  }
 })();
}
window.addEventListener('pagehide',unsubscribe_updates);

export async function speech_text(language) {
 if(!window.isSecureContext) throw new Error('Speech requires HTTPS. Open the private HTTPS cafe link, then allow the microphone.');
 const API=window.SpeechRecognition||window.webkitSpeechRecognition;
 if(!API) throw new Error('Speech recognition is unavailable in this browser. Use keyboard dictation or type your message.');
 return new Promise((resolve,reject)=>{
  const r=new API();recognition=r;r.continuous=false;r.interimResults=false;
  r.lang=({en:'en-US',ko:'ko-KR',yue:'zh-HK',zh:'zh-CN',ja:'ja-JP',cs:'cs-CZ',es:'es-ES'})[language]||'en-US';
  let settled=false;
  r.onresult=e=>{settled=true;resolve(e.results[0][0].transcript);};
  r.onerror=e=>{settled=true;if(e.error==='aborted')resolve('');else reject(new Error('Speech: '+e.error+'. Check microphone permission or type your message.'));};
  r.onend=()=>{recognition=null;if(!settled)resolve('');};r.start();
 });
}

export function scroll_chat(){requestAnimationFrame(()=>{const log=document.querySelector('.chat-messages');if(log)log.scrollTop=log.scrollHeight;});}

export function new_request_id() { return Array.from(crypto.getRandomValues(new Uint32Array(4)), n => n.toString(16)).join("-"); }
