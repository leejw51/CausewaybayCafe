#!/usr/bin/env python3
"""Convert a completed recording to a three-minute H.264 MP4 with optional subtitles."""
import argparse,json,subprocess,textwrap
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('video',type=Path)
parser.add_argument('--chapters',type=Path,default=Path('artifacts/demo/chapters.json'))
parser.add_argument('--output',type=Path,default=Path('artifacts/demo/causewaybay-muse-demo.mp4'))
args=parser.parse_args()
marks=json.loads(args.chapters.read_text())
def timestamp(seconds):
 milliseconds=round(seconds*1000)
 return f'{milliseconds//3600000:02}:{milliseconds//60000%60:02}:{milliseconds//1000%60:02},{milliseconds%1000:03}'
subtitles=args.output.with_suffix('.srt')
lines=[]
for index,mark in enumerate(marks):
 start=min(180,mark['seconds']);end=min(180,marks[index+1]['seconds'] if index+1<len(marks) else 180)
 if end<=start:continue
 lines.extend([str(index+1),f'{timestamp(start)} --> {timestamp(end)}','\n'.join(textwrap.wrap(mark['text'],65)),''])
subtitles.write_text('\n'.join(lines),encoding='utf-8')
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(args.video),'-i',str(subtitles),'-t','180','-map','0:v:0','-map','1:0','-c:v','libx264','-preset','fast','-crf','20','-pix_fmt','yuv420p','-c:s','mov_text','-metadata:s:s:0','language=eng','-disposition:s:0','default','-movflags','+faststart',str(args.output)],check=True)
print(args.output)
subprocess.run(['ffprobe','-v','error','-show_entries','format=duration,size:stream=codec_name,width,height','-of','json',str(args.output)],check=True)
